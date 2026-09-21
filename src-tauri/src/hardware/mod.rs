//! Hardware monitor: static configuration and live readings, sampled on demand while the UI shows
//! them (nothing runs in the background).
//!
//! CPU temperatures come from `hangar-sensors.exe`, a small elevated helper that reads them through
//! the PawnIO driver and writes `data\sensors.json` (see the `sensors` crate).

mod gpu;
mod pdh;

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use nvml_wrapper::Nvml;
use serde::{Deserialize, Serialize};
use sysinfo::{DiskKind, Disks, Motherboard, Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub use gpu::Adapter;

/// Readings older than this come from a sensor that stopped.
const SENSOR_STALE_SECS: u64 = 10;
pub const PAWNIO_LIB: &str = r"C:\Program Files\PawnIO\PawnIOLib.dll";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub physical_cores: Option<usize>,
    pub logical_cores: usize,
    pub memory_total: u64,
    pub os: Option<String>,
    pub motherboard: Option<String>,
    pub gpus: Vec<Adapter>,
    pub disks: Vec<DiskInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub mount: String,
    pub name: String,
    pub kind: &'static str,
    pub total: u64,
    pub available: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub cpu: CpuReading,
    pub memory_used: u64,
    pub memory_total: u64,
    pub gpus: Vec<GpuReading>,
    pub disk_read_bps: Option<f64>,
    pub disk_write_bps: Option<f64>,
    pub net_rx_bps: f64,
    pub net_tx_bps: f64,
    pub battery: Option<Battery>,
    /// ACPI thermal zone: a slow motherboard probe, shown when the CPU sensor is off.
    pub system_temp_c: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuReading {
    pub usage: f32,
    pub per_core: Vec<f32>,
    pub frequency_mhz: u64,
    pub package_c: Option<f32>,
    pub cores_c: Vec<Option<f32>>,
    pub power_w: Option<f32>,
    pub sensor: SensorStatus,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SensorStatus {
    /// PawnIO is not installed.
    NoDriver,
    /// The driver is there but the helper isn't running (task not created yet).
    Off,
    Running,
    Error { message: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuReading {
    pub name: String,
    pub vendor: &'static str,
    pub usage: Option<f64>,
    /// Process using this GPU the most, e.g. "RDR2.exe" — shows which GPU a game runs on.
    pub top_process: Option<String>,
    #[serde(flatten)]
    pub nvidia: gpu::NvidiaDetails,
    /// NVIDIA chip powered down (Optimus): not queried, to avoid waking it up.
    pub asleep: bool,
}

pub struct SessionSample {
    pub cpu_load: f64,
    pub cpu_temp: Option<f64>,
    pub gpu_temp: Option<f64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    pub percent: u8,
    pub plugged_in: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SensorFile {
    updated_at: u64,
    package_c: Option<f32>,
    #[serde(default)]
    cores_c: Vec<Option<f32>>,
    package_power_w: Option<f32>,
    error: Option<String>,
}

pub struct Monitor {
    system: System,
    networks: Networks,
    last_sample: Instant,
    adapters: Vec<Adapter>,
    nvml: Option<Nvml>,
    pdh: Option<pdh::Query>,
    gpu_engine: Option<pdh::Counter>,
    disk_read: Option<pdh::Counter>,
    disk_write: Option<pdh::Counter>,
    thermal: Option<pdh::Counter>,
    sensors_file: PathBuf,
}

impl Monitor {
    pub fn new(data_dir: &Path) -> Self {
        let pdh = pdh::Query::open();
        let add = |path: &str| pdh.as_ref().and_then(|q| q.add(path));
        let monitor = Self {
            system: System::new(),
            networks: Networks::new_with_refreshed_list(),
            last_sample: Instant::now(),
            adapters: gpu::adapters(),
            nvml: Nvml::init().ok(),
            gpu_engine: add(r"\GPU Engine(*)\Utilization Percentage"),
            disk_read: add(r"\PhysicalDisk(_Total)\Disk Read Bytes/sec"),
            disk_write: add(r"\PhysicalDisk(_Total)\Disk Write Bytes/sec"),
            thermal: add(r"\Thermal Zone Information(*)\High Precision Temperature"),
            pdh,
            sensors_file: data_dir.join("sensors.json"),
        };
        if let Some(q) = &monitor.pdh {
            q.collect(); // first sample: rate counters report from the second one
        }
        monitor
    }

    pub fn info(&self) -> HardwareInfo {
        let mut system = System::new();
        system.refresh_cpu_all();
        system.refresh_memory();
        let disks = Disks::new_with_refreshed_list()
            .iter()
            .map(|d| DiskInfo {
                mount: d.mount_point().display().to_string(),
                name: d.name().to_string_lossy().into_owned(),
                kind: match d.kind() {
                    DiskKind::SSD => "ssd",
                    DiskKind::HDD => "hdd",
                    _ => "other",
                },
                total: d.total_space(),
                available: d.available_space(),
            })
            .collect();
        HardwareInfo {
            cpu_name: system.cpus().first().map(|c| c.brand().trim().to_string()).unwrap_or_default(),
            physical_cores: System::physical_core_count(),
            logical_cores: system.cpus().len(),
            memory_total: system.total_memory(),
            os: System::long_os_version(),
            motherboard: Motherboard::new().and_then(|m| match (m.vendor_name(), m.name()) {
                (Some(v), Some(n)) => Some(format!("{v} {n}")),
                (v, n) => v.or(n),
            }),
            gpus: self.adapters.clone(),
            disks,
        }
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let elapsed = self.last_sample.elapsed().as_secs_f64().max(0.001);
        self.last_sample = Instant::now();
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.networks.refresh(true);
        if let Some(q) = &self.pdh {
            q.collect();
        }

        let (rx, tx) = self.networks.values().fold((0, 0), |(r, t), n| (r + n.received(), t + n.transmitted()));
        let pdh_values = |counter: &Option<pdh::Counter>| match (&self.pdh, counter) {
            (Some(q), Some(c)) => q.values(c),
            _ => vec![],
        };
        let first = |counter: &Option<pdh::Counter>| pdh_values(counter).first().map(|(_, v)| *v);
        let engine_samples = pdh_values(&self.gpu_engine);
        let disk_read_bps = first(&self.disk_read);
        let disk_write_bps = first(&self.disk_write);
        // Tenths of kelvin.
        let system_temp_c = first(&self.thermal).map(|k| (k / 10.0 - 273.15).round());

        Snapshot {
            cpu: self.cpu(),
            memory_used: self.system.used_memory(),
            memory_total: self.system.total_memory(),
            gpus: self.gpus(&engine_samples),
            disk_read_bps,
            disk_write_bps,
            net_rx_bps: rx as f64 / elapsed,
            net_tx_bps: tx as f64 / elapsed,
            battery: battery(),
            system_temp_c,
        }
    }

    /// Readings kept with a play session: CPU load, hottest CPU and GPU temperatures.
    pub fn session_sample(&mut self) -> SessionSample {
        let snap = self.snapshot();
        SessionSample {
            cpu_load: snap.cpu.usage as f64,
            cpu_temp: snap.cpu.package_c.map(f64::from).or(snap.system_temp_c),
            gpu_temp: snap.gpus.iter().filter_map(|g| g.nvidia.temperature_c).max().map(f64::from),
        }
    }

    fn cpu(&self) -> CpuReading {
        let cpus = self.system.cpus();
        let sensor = self.read_sensor_file();
        let (status, package_c, cores_c, power_w) = match sensor {
            Ok(Some(file)) => (SensorStatus::Running, file.package_c, file.cores_c, file.package_power_w),
            Ok(None) if Path::new(PAWNIO_LIB).exists() => (SensorStatus::Off, None, vec![], None),
            Ok(None) => (SensorStatus::NoDriver, None, vec![], None),
            Err(message) => (SensorStatus::Error { message }, None, vec![], None),
        };
        CpuReading {
            usage: self.system.global_cpu_usage(),
            per_core: cpus.iter().map(|c| c.cpu_usage()).collect(),
            frequency_mhz: cpus.iter().map(|c| c.frequency()).max().unwrap_or(0),
            package_c,
            cores_c,
            power_w,
            sensor: status,
        }
    }

    /// Fresh sensor readings, `None` when the helper isn't running.
    fn read_sensor_file(&self) -> Result<Option<SensorFile>, String> {
        let Ok(text) = std::fs::read_to_string(&self.sensors_file) else { return Ok(None) };
        let Ok(file) = serde_json::from_str::<SensorFile>(&text) else { return Ok(None) };
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if let Some(error) = file.error {
            return Err(error);
        }
        Ok((now.saturating_sub(file.updated_at) <= SENSOR_STALE_SECS).then_some(file))
    }

    fn gpus(&mut self, engine_samples: &[(String, f64)]) -> Vec<GpuReading> {
        let load = gpu::engine_load(engine_samples);
        let top_pids: Vec<Pid> = load.top_process.values().map(|(pid, _)| Pid::from_u32(*pid)).collect();
        if !top_pids.is_empty() {
            self.system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&top_pids),
                true,
                ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
            );
        }
        let pdh_available = self.gpu_engine.is_some();

        self.adapters
            .iter()
            .map(|adapter| {
                let usage = load.by_adapter.get(&adapter.luid).copied().or(pdh_available.then_some(0.0));
                let top_process = load
                    .top_process
                    .get(&adapter.luid)
                    .filter(|(_, busy)| *busy >= 1.0)
                    .and_then(|(pid, _)| self.system.process(Pid::from_u32(*pid)))
                    .map(|p| p.name().to_string_lossy().into_owned());
                // Querying NVML powers an idle Optimus GPU back up: only ask when it's already working.
                let active = usage.is_some_and(|u| u > 0.0);
                let nvidia = match (&self.nvml, adapter.vendor) {
                    (Some(nvml), "nvidia") if active || !pdh_available => {
                        gpu::nvidia_details(nvml).unwrap_or_default()
                    }
                    _ => gpu::NvidiaDetails::default(),
                };
                GpuReading {
                    name: adapter.name.clone(),
                    vendor: adapter.vendor,
                    usage,
                    top_process,
                    asleep: adapter.vendor == "nvidia" && pdh_available && !active,
                    nvidia,
                }
            })
            .collect()
    }
}

#[cfg(windows)]
fn battery() -> Option<Battery> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return None;
    }
    // 128: no system battery; 255: unknown.
    if status.BatteryFlag & 128 != 0 || status.BatteryLifePercent > 100 {
        return None;
    }
    Some(Battery { percent: status.BatteryLifePercent, plugged_in: status.ACLineStatus == 1 })
}

#[cfg(not(windows))]
fn battery() -> Option<Battery> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real readings on this PC: `cargo test live_hardware -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_hardware() {
        let mut monitor = Monitor::new(&std::env::temp_dir());
        println!("{}", serde_json::to_string_pretty(&monitor.info()).unwrap());
        monitor.snapshot();
        std::thread::sleep(std::time::Duration::from_secs(1));
        println!("{}", serde_json::to_string_pretty(&monitor.snapshot()).unwrap());
    }
}
