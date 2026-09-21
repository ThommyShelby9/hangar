//! Controllers, read through SDL3 on a dedicated thread.
//!
//! Hangar coexists with DS4Windows: it only *reads* controllers (buttons, sticks, battery…) and
//! never changes their mode or lightbar. Rumble and motion sensors, which switch a Bluetooth
//! PlayStation controller into SDL's "enhanced" mode, only run when the user asks for them.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sdl3::gamepad::{Axis, Button, Gamepad, GamepadType};
use sdl3::joystick::{ConnectionState, PowerLevel};
use sdl3::sensor::SensorType;
use serde::Serialize;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Emitter};

/// Emitted when a controller connects, disconnects, or its battery changes.
pub const CONTROLLERS_CHANGED: &str = "controllers-changed";
/// Emitted ~30 times per second while the test page is open.
pub const CONTROLLER_INPUT: &str = "controller-input";

const LIVE_INTERVAL: Duration = Duration::from_millis(33);
const IDLE_INTERVAL: Duration = Duration::from_millis(250);
const STATUS_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PadInfo {
    pub id: u32,
    pub name: String,
    /// "ps4", "ps3", "ps5", "xbox", "switch" or "other".
    pub kind: &'static str,
    /// "usb", "bluetooth" or "unknown".
    pub connection: &'static str,
    pub battery: Option<PadBattery>,
    pub has_rumble: bool,
    pub has_touchpad: bool,
    pub has_motion: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PadBattery {
    pub percent: Option<u8>,
    pub charging: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub pads: Vec<PadInfo>,
    /// DS4Windows owns the lightbar and Xbox emulation while it runs.
    pub ds4windows: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PadInput {
    id: u32,
    buttons: Vec<&'static str>,
    left: [f32; 2],
    right: [f32; 2],
    triggers: [f32; 2],
    fingers: Vec<[f32; 2]>,
    gyro: Option<[f32; 3]>,
    accel: Option<[f32; 3]>,
}

pub enum Command {
    /// The test page is open: stream inputs.
    Live(bool),
    Rumble { id: u32, low: u16, high: u16, ms: u32 },
    Motion { id: u32, enabled: bool },
}

pub struct Controllers {
    pub commands: Mutex<Sender<Command>>,
    pub status: Arc<Mutex<Status>>,
}

pub fn spawn(app: AppHandle) -> Controllers {
    let (tx, rx) = channel();
    let status = Arc::new(Mutex::new(Status::default()));
    let shared = status.clone();
    std::thread::Builder::new()
        .name("hangar-controllers".into())
        .spawn(move || {
            if let Err(e) = run(&app, rx, &shared) {
                eprintln!("Hangar : manettes indisponibles : {e}");
            }
        })
        .expect("impossible de démarrer le thread des manettes");
    Controllers { commands: Mutex::new(tx), status }
}

const BUTTONS: &[(Button, &str)] = &[
    (Button::South, "south"),
    (Button::East, "east"),
    (Button::West, "west"),
    (Button::North, "north"),
    (Button::LeftShoulder, "l1"),
    (Button::RightShoulder, "r1"),
    (Button::LeftStick, "l3"),
    (Button::RightStick, "r3"),
    (Button::Back, "share"),
    (Button::Start, "options"),
    (Button::Guide, "ps"),
    (Button::Touchpad, "touchpad"),
    (Button::DPadUp, "up"),
    (Button::DPadDown, "down"),
    (Button::DPadLeft, "left"),
    (Button::DPadRight, "right"),
];

fn run(app: &AppHandle, rx: Receiver<Command>, status: &Arc<Mutex<Status>>) -> Result<(), String> {
    // Hangar has no SDL window: without this, SDL ignores controllers while another window has focus.
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    // Never switch a PlayStation controller into enhanced mode on our own (it breaks DirectInput games).
    sdl3::hint::set("SDL_JOYSTICK_ENHANCED_REPORTS", "auto");
    sdl3::hint::set("SDL_JOYSTICK_THREAD", "1");

    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let subsystem = sdl.gamepad().map_err(|e| e.to_string())?;
    let mut events = sdl.event_pump().map_err(|e| e.to_string())?;

    let mut pads: Vec<Gamepad> = Vec::new();
    let mut live = false;
    let mut last_status = Instant::now() - STATUS_INTERVAL;
    let mut system = System::new();

    loop {
        for command in rx.try_iter() {
            match command {
                Command::Live(on) => live = on,
                Command::Rumble { id, low, high, ms } => {
                    if let Some(pad) = pads.iter_mut().find(|p| pad_id(p) == id) {
                        let _ = pad.set_rumble(low, high, ms);
                    }
                }
                Command::Motion { id, enabled } => {
                    if let Some(pad) = pads.iter_mut().find(|p| pad_id(p) == id) {
                        for sensor in [SensorType::Gyroscope, SensorType::Accelerometer] {
                            let _ = pad.sensor_set_enabled(sensor, enabled);
                        }
                    }
                }
            }
        }

        let mut changed = false;
        for event in events.poll_iter() {
            match event {
                sdl3::event::Event::GamepadAdded { which, .. } => {
                    if !pads.iter().any(|p| pad_id(p) == which.raw()) {
                        if let Ok(pad) = subsystem.open(which) {
                            pads.push(pad);
                            changed = true;
                        }
                    }
                }
                sdl3::event::Event::GamepadRemoved { which, .. } => {
                    pads.retain(|p| pad_id(p) != which.raw());
                    changed = true;
                }
                _ => {}
            }
        }

        if changed || last_status.elapsed() >= STATUS_INTERVAL {
            last_status = Instant::now();
            system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
            let next = Status {
                pads: pads.iter().map(info).collect(),
                ds4windows: system.processes().values().any(|p| {
                    p.name().to_string_lossy().eq_ignore_ascii_case("DS4Windows.exe")
                }),
            };
            let mut current = status.lock().map_err(|e| e.to_string())?;
            if current.pads != next.pads || current.ds4windows != next.ds4windows {
                *current = next.clone();
                let _ = app.emit(CONTROLLERS_CHANGED, next);
            }
        }

        if live && !pads.is_empty() {
            let inputs: Vec<PadInput> = pads.iter().map(input).collect();
            let _ = app.emit(CONTROLLER_INPUT, inputs);
        }
        std::thread::sleep(if live { LIVE_INTERVAL } else { IDLE_INTERVAL });
    }
}

fn pad_id(pad: &Gamepad) -> u32 {
    pad.id().map(|id| id.raw()).unwrap_or(0)
}

fn info(pad: &Gamepad) -> PadInfo {
    let kind = match pad.real_type() {
        GamepadType::PS4 => "ps4",
        GamepadType::PS3 => "ps3",
        GamepadType::PS5 => "ps5",
        GamepadType::Xbox360 | GamepadType::XboxOne => "xbox",
        GamepadType::NintendoSwitchPro
        | GamepadType::NintendoSwitchJoyconLeft
        | GamepadType::NintendoSwitchJoyconRight
        | GamepadType::NintendoSwitchJoyconPair => "switch",
        _ => "other",
    };
    let connection = match pad.connection_state() {
        Ok(ConnectionState::Wired) => "usb",
        Ok(ConnectionState::Wireless) => "bluetooth",
        _ => "unknown",
    };
    let power = pad.power_info();
    let battery = match power.state {
        PowerLevel::OnBattery | PowerLevel::Charging | PowerLevel::Charged => Some(PadBattery {
            percent: u8::try_from(power.percentage).ok(),
            charging: matches!(power.state, PowerLevel::Charging),
        }),
        _ => None,
    };
    PadInfo {
        id: pad_id(pad),
        name: pad.name().unwrap_or_else(|| "Manette".into()),
        kind,
        connection,
        battery,
        has_rumble: true,
        has_touchpad: pad.touchpads_count() > 0,
        has_motion: unsafe { pad.has_sensor(SensorType::Gyroscope) },
    }
}

fn input(pad: &Gamepad) -> PadInput {
    let axis = |a: Axis| pad.axis(a) as f32 / 32767.0;
    let mut fingers = Vec::new();
    if pad.touchpads_count() > 0 {
        for finger in 0..pad.supported_touchpad_fingers(0) as i32 {
            let (mut down, mut x, mut y, mut pressure) = (false, 0.0f32, 0.0f32, 0.0f32);
            let ok = unsafe {
                sdl3::sys::gamepad::SDL_GetGamepadTouchpadFinger(pad.raw(), 0, finger, &mut down, &mut x, &mut y, &mut pressure)
            };
            if ok && down {
                fingers.push([x, y]);
            }
        }
    }
    let sensor = |kind: SensorType| {
        if !pad.sensor_enabled(kind) {
            return None;
        }
        let mut data = [0f32; 3];
        pad.sensor_get_data(kind, &mut data).ok().map(|_| data)
    };
    PadInput {
        id: pad_id(pad),
        buttons: BUTTONS.iter().filter(|(b, _)| pad.button(*b)).map(|(_, name)| *name).collect(),
        left: [axis(Axis::LeftX), axis(Axis::LeftY)],
        right: [axis(Axis::RightX), axis(Axis::RightY)],
        triggers: [axis(Axis::TriggerLeft), axis(Axis::TriggerRight)],
        fingers,
        gyro: sensor(SensorType::Gyroscope),
        accel: sensor(SensorType::Accelerometer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Controllers visible to Hangar on this PC: `cargo test live_controllers -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_controllers() {
        sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        sdl3::hint::set("SDL_JOYSTICK_ENHANCED_REPORTS", "auto");
        let sdl = sdl3::init().unwrap();
        let subsystem = sdl.gamepad().unwrap();
        let mut events = sdl.event_pump().unwrap();
        let start = Instant::now();
        let mut pads = Vec::new();
        while start.elapsed() < Duration::from_secs(2) {
            for event in events.poll_iter() {
                if let sdl3::event::Event::GamepadAdded { which, .. } = event {
                    pads.push(subsystem.open(which).unwrap());
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        for pad in &pads {
            println!("{:?}", info(pad));
        }
        println!("{} manette(s)", pads.len());
    }
}
