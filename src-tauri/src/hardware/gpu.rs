//! Graphics adapters: names and memory from DXGI, load per adapter and per process from the
//! "GPU Engine" counters, and details for NVIDIA cards from NVML.

use std::collections::HashMap;

use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::Nvml;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub name: String,
    pub vendor: &'static str,
    pub dedicated_memory: u64,
    /// "0x{high:08x}_0x{low:08x}", as it appears in GPU Engine counter instance names.
    #[serde(skip)]
    pub luid: String,
}

/// Hardware adapters (software renderers such as "Microsoft Basic Render Driver" are skipped).
pub fn adapters() -> Vec<Adapter> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    let Ok(factory) = (unsafe { CreateDXGIFactory1::<IDXGIFactory1>() }) else { return vec![] };
    let mut list = Vec::new();
    for i in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else { break };
        let Ok(desc) = (unsafe { adapter.GetDesc1() }) else { continue };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
            continue;
        }
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
        let name = String::from_utf16_lossy(&desc.Description[..len]).trim().to_string();
        let vendor = match desc.VendorId {
            0x10DE => "nvidia",
            0x8086 => "intel",
            0x1002 | 0x1022 => "amd",
            _ => "other",
        };
        list.push(Adapter {
            name,
            vendor,
            dedicated_memory: desc.DedicatedVideoMemory as u64,
            luid: format!("0x{:08x}_0x{:08x}", desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart),
        });
    }
    list
}

/// Load per adapter (luid → %) and, per adapter, the busiest process (pid, %).
pub struct EngineLoad {
    pub by_adapter: HashMap<String, f64>,
    pub top_process: HashMap<String, (u32, f64)>,
}

/// Parses "pid_1234_luid_0x00000000_0x0000D3D2_phys_0_eng_3_engtype_3D" samples.
/// Like Task Manager: an adapter's load is its busiest engine, summed over processes.
pub fn engine_load(samples: &[(String, f64)]) -> EngineLoad {
    let mut per_engine: HashMap<(String, String), f64> = HashMap::new();
    let mut per_process: HashMap<(String, u32), f64> = HashMap::new();
    for (instance, value) in samples {
        let parts: Vec<&str> = instance.split('_').collect();
        let field = |key: &str| parts.iter().position(|p| *p == key).and_then(|i| parts.get(i + 1));
        let (Some(pid), Some(luid_hi), Some(eng)) = (field("pid"), field("luid"), field("eng")) else { continue };
        let Some(luid_lo) = parts.iter().position(|p| *p == "luid").and_then(|i| parts.get(i + 2)) else { continue };
        let luid = format!("{luid_hi}_{luid_lo}").to_lowercase();
        *per_engine.entry((luid.clone(), eng.to_string())).or_default() += value;
        if let Ok(pid) = pid.parse() {
            *per_process.entry((luid, pid)).or_default() += value;
        }
    }
    let mut by_adapter: HashMap<String, f64> = HashMap::new();
    for ((luid, _), load) in per_engine {
        let entry = by_adapter.entry(luid).or_default();
        *entry = entry.max(load.min(100.0));
    }
    let mut top_process: HashMap<String, (u32, f64)> = HashMap::new();
    for ((luid, pid), load) in per_process {
        let entry = top_process.entry(luid).or_insert((pid, 0.0));
        if load > entry.1 {
            *entry = (pid, load);
        }
    }
    EngineLoad { by_adapter, top_process }
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaDetails {
    pub temperature_c: Option<u32>,
    pub memory_used: Option<u64>,
    pub memory_total: Option<u64>,
    pub clock_mhz: Option<u32>,
    pub power_w: Option<f32>,
}

/// Details of the first NVIDIA GPU. Each value is optional: mobile GeForce chips don't report all of them.
pub fn nvidia_details(nvml: &Nvml) -> Option<NvidiaDetails> {
    let device = nvml.device_by_index(0).ok()?;
    let memory = device.memory_info().ok();
    Some(NvidiaDetails {
        temperature_c: device.temperature(TemperatureSensor::Gpu).ok(),
        memory_used: memory.as_ref().map(|m| m.used),
        memory_total: memory.as_ref().map(|m| m.total),
        clock_mhz: device.clock_info(Clock::Graphics).ok(),
        power_w: device.power_usage().ok().map(|mw| mw as f32 / 1000.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busiest_engine_and_process_per_adapter() {
        let samples = vec![
            ("pid_10_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D".to_string(), 30.0),
            ("pid_20_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D".to_string(), 25.0),
            ("pid_20_luid_0x00000000_0x0000AAAA_phys_0_eng_3_engtype_VideoDecode".to_string(), 4.0),
            ("pid_30_luid_0x00000000_0x0000BBBB_phys_0_eng_0_engtype_3D".to_string(), 2.0),
        ];
        let load = engine_load(&samples);
        assert_eq!(load.by_adapter["0x00000000_0x0000aaaa"], 55.0);
        assert_eq!(load.by_adapter["0x00000000_0x0000bbbb"], 2.0);
        assert_eq!(load.top_process["0x00000000_0x0000aaaa"], (10, 30.0));
    }
}
