//! hangar-sensors: reads Intel CPU temperatures and package power through PawnIO and writes them to
//! `<exe folder>\data\sensors.json` every two seconds, for Hangar to display.
//!
//! PawnIO only accepts administrators, so this small program runs elevated through a scheduled task
//! created once with `--install`; Hangar itself (and the games it launches) stays unelevated.
//!
//!   hangar-sensors.exe             run the sensor loop
//!   hangar-sensors.exe --install   create the logon task (needs admin) and start it
//!   hangar-sensors.exe --uninstall remove the task and stop the sensor

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pawnio;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

const TASK_NAME: &str = "Hangar - capteurs";
const INTERVAL: Duration = Duration::from_secs(2);

const MSR_RAPL_POWER_UNIT: u32 = 0x606;
const MSR_PKG_ENERGY_STATUS: u32 = 0x611;
const MSR_IA32_THERM_STATUS: u32 = 0x19C;
const MSR_IA32_TEMPERATURE_TARGET: u32 = 0x1A2;
const MSR_IA32_PACKAGE_THERM_STATUS: u32 = 0x1B1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Reading {
    updated_at: u64,
    package_c: Option<f32>,
    cores_c: Vec<Option<f32>>,
    package_power_w: Option<f32>,
    error: Option<String>,
}

fn main() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let result = match std::env::args().nth(1).as_deref() {
        Some("--install") => install(&exe_dir),
        Some("--uninstall") => uninstall(),
        _ => run(&exe_dir),
    };
    if let Err(e) = result {
        eprintln!("hangar-sensors : {e}");
        std::process::exit(1);
    }
}

fn output_path(exe_dir: &Path) -> PathBuf {
    exe_dir.join("data").join("sensors.json")
}

fn write(exe_dir: &Path, reading: &Reading) {
    let path = output_path(exe_dir);
    let tmp = path.with_extension("json.tmp");
    if let Ok(json) = serde_json::to_vec(reading) {
        let _ = std::fs::create_dir_all(exe_dir.join("data"));
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

fn run(exe_dir: &Path) -> Result<(), String> {
    // One sensor at a time, even if the task and a manual start overlap.
    let _guard = single_instance().ok_or("déjà en cours")?;

    let io = match open_driver(exe_dir) {
        Ok(io) => io,
        Err(e) => {
            write(exe_dir, &Reading { updated_at: now(), package_c: None, cores_c: vec![], package_power_w: None, error: Some(e.clone()) });
            return Err(e);
        }
    };

    let tj_max = io.read_msr(MSR_IA32_TEMPERATURE_TARGET).map(|v| ((v >> 16) & 0xFF) as f32).filter(|t| *t > 0.0).unwrap_or(100.0);
    let energy_unit = io.read_msr(MSR_RAPL_POWER_UNIT).map(|v| 1.0 / (1u64 << ((v >> 8) & 0x1F)) as f64);
    let cpus = logical_cpu_count();
    let mut last_energy: Option<(u64, Instant)> = None;

    loop {
        let temp = |raw: u64| (raw & (1 << 31) != 0).then(|| tj_max - ((raw >> 16) & 0x7F) as f32);
        let package_c = io.read_msr(MSR_IA32_PACKAGE_THERM_STATUS).and_then(temp);
        let cores_c = (0..cpus).map(|cpu| on_cpu(cpu, || io.read_msr(MSR_IA32_THERM_STATUS)).and_then(temp)).collect();

        let package_power_w = match (energy_unit, io.read_msr(MSR_PKG_ENERGY_STATUS)) {
            (Some(unit), Some(raw)) => {
                let raw = raw & 0xFFFF_FFFF;
                let now = Instant::now();
                let power = last_energy.map(|(prev, at)| {
                    let delta = raw.wrapping_sub(prev) & 0xFFFF_FFFF; // 32-bit counter wraps
                    (delta as f64 * unit / now.duration_since(at).as_secs_f64()) as f32
                });
                last_energy = Some((raw, now));
                power
            }
            _ => None,
        };

        write(exe_dir, &Reading { updated_at: now(), package_c, cores_c, package_power_w, error: None });
        std::thread::sleep(INTERVAL);
    }
}

fn open_driver(exe_dir: &Path) -> Result<pawnio::PawnIo, String> {
    if !is_intel() {
        return Err("seuls les processeurs Intel sont pris en charge pour l'instant".into());
    }
    pawnio::PawnIo::open(&exe_dir.join("IntelMSR.bin"))
}

fn is_intel() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        let r = std::arch::x86_64::__cpuid(0);
        let vendor = [r.ebx.to_le_bytes(), r.edx.to_le_bytes(), r.ecx.to_le_bytes()].concat();
        vendor == b"GenuineIntel"
    }
    #[cfg(not(target_arch = "x86_64"))]
    false
}

fn logical_cpu_count() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(64)
}

/// Runs `f` on logical CPU `cpu`: MSRs are per core.
fn on_cpu<T>(cpu: usize, f: impl FnOnce() -> T) -> T {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadAffinityMask};
    unsafe {
        let thread = GetCurrentThread();
        let previous = SetThreadAffinityMask(thread, 1usize << cpu);
        std::thread::yield_now();
        let result = f();
        if previous != 0 {
            SetThreadAffinityMask(thread, previous);
        }
        result
    }
}

fn single_instance() -> Option<windows_sys::Win32::Foundation::HANDLE> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let name: Vec<u16> = "Local\\HangarSensors".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        (!handle.is_null() && GetLastError() != ERROR_ALREADY_EXISTS).then_some(handle)
    }
}

/// `schtasks` without a console window flashing on screen.
fn schtasks(args: &[&str]) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map(|s| s.success())
        .map_err(|e| e.to_string())
}

/// Registers the logon task (run elevated, no prompt afterwards) and starts it now.
/// Defined in XML because the command line can't lift the defaults that would break a laptop:
/// "don't start on battery" and "stop after 3 days".
fn install(exe_dir: &Path) -> Result<(), String> {
    let exe = exe_dir.join("hangar-sensors.exe");
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Température du processeur pour Hangar (PawnIO).</Description></RegistrationInfo>
  <Triggers><LogonTrigger><Enabled>true</Enabled></LogonTrigger></Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>HighestAvailable</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Hidden>true</Hidden>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>{}</Command></Exec></Actions>
</Task>"#,
        exe.display()
    );
    // schtasks wants UTF-16 with a byte-order mark; the file stays in Hangar's own data folder.
    let xml_path = exe_dir.join("data").join("hangar-sensors-task.xml");
    let _ = std::fs::create_dir_all(exe_dir.join("data"));
    let bytes: Vec<u8> = std::iter::once(0xFEFFu16)
        .chain(xml.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    std::fs::write(&xml_path, bytes).map_err(|e| e.to_string())?;
    let created = schtasks(&["/Create", "/F", "/TN", TASK_NAME, "/XML", &xml_path.display().to_string()]);
    let _ = std::fs::remove_file(&xml_path);
    if !created? {
        return Err("création de la tâche planifiée refusée".into());
    }
    schtasks(&["/Run", "/TN", TASK_NAME])?;
    Ok(())
}

fn uninstall() -> Result<(), String> {
    let _ = schtasks(&["/End", "/TN", TASK_NAME]);
    schtasks(&["/Delete", "/F", "/TN", TASK_NAME])?;
    Ok(())
}
