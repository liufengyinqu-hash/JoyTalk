//! Joy-Con scanner + per-device input loop with payload dispatch and ControllerDetected emit.

use joycon_rs::prelude::input_report_mode::standard_full_mode::IMUData;
use joycon_rs::prelude::input_report_mode::{BatteryLevel, StandardInputReport};
use joycon_rs::prelude::*;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::actions::{dispatch, ActionRegistry};
use crate::settings::save;
use crate::state::State;
use crate::types::{
    ActionPayload, ButtonMapping, ControllerDetected, ControllerKind, JoyConActionFired,
    JoyConButton, JoyConButtonEvent, JoyConSide, JoyConStatus, TriggerMode,
};

const POLL: Duration = Duration::from_millis(8);
const READ_TIMEOUT_MS: i32 = 100;
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const BATTERY_EMIT_INTERVAL: Duration = Duration::from_secs(10);
const WARMUP: Duration = Duration::from_millis(300);

const TAP_THRESHOLD: Duration = Duration::from_millis(220);
const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(380);
const LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(800);
const REPEAT_INITIAL_DELAY: Duration = Duration::from_millis(350);
const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

// Analog stick is 12-bit (0..=4095) with center near 2048. Treat tilt > 500
// from center as "active" so the schematic can highlight the stick when
// the user moves it (in addition to the click event).
const STICK_CENTER: i32 = 2048;
const STICK_TILT_THRESHOLD: i32 = 500;

const ALL_BUTTONS: &[JoyConButton] = &[
    JoyConButton::A, JoyConButton::B, JoyConButton::X, JoyConButton::Y,
    JoyConButton::Plus, JoyConButton::Minus, JoyConButton::Home, JoyConButton::Capture,
    JoyConButton::L, JoyConButton::R, JoyConButton::Zl, JoyConButton::Zr,
    JoyConButton::LStick, JoyConButton::RStick,
    JoyConButton::SlLeft, JoyConButton::SrLeft, JoyConButton::SlRight, JoyConButton::SrRight,
    JoyConButton::Up, JoyConButton::Down, JoyConButton::Left, JoyConButton::Right,
    JoyConButton::LStickUp, JoyConButton::LStickDown,
    JoyConButton::LStickLeft, JoyConButton::LStickRight,
    JoyConButton::RStickUp, JoyConButton::RStickDown,
    JoyConButton::RStickLeft, JoyConButton::RStickRight,
    JoyConButton::Shake, JoyConButton::FlipUp, JoyConButton::FlipDown,
    JoyConButton::TiltLeft, JoyConButton::TiltRight,
    JoyConButton::ShakeHorizontal, JoyConButton::ShakeVertical,
];

// Stick directional threshold (more sensitive than tilt threshold for click)
const STICK_DIR_THRESHOLD: i32 = 800;

pub fn spawn_scanner<R: Runtime>(
    app: AppHandle<R>,
    state: State,
    registry: Arc<ActionRegistry<R>>,
) {
    thread::spawn(move || scanner_loop(app, state, registry));
}

fn scanner_loop<R: Runtime>(
    app: AppHandle<R>,
    state: State,
    registry: Arc<ActionRegistry<R>>,
) {
    info!("[joycon] scanner started");
    let manager = JoyConManager::get_instance();
    let new_devices_rx = match manager.lock() {
        Ok(m) => m.new_devices(),
        Err(_) => {
            warn!("[joycon] manager poisoned");
            return;
        }
    };

    let mut device_idx: u8 = 0;
    while state.running.load(Ordering::Relaxed) {
        match new_devices_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(dev) => {
                let idx = device_idx;
                device_idx = device_idx.wrapping_add(1);
                let app_c = app.clone();
                let state_c = state.clone();
                let reg_c = registry.clone();
                thread::spawn(move || device_loop(app_c, state_c, reg_c, dev, idx));
            }
            Err(_) => {
                let elapsed = state
                    .last_seen
                    .lock()
                    .map(|g| g.elapsed())
                    .unwrap_or(Duration::ZERO);
                if state.connected.load(Ordering::Relaxed) && elapsed > STATUS_TIMEOUT {
                    state.connected.store(false, Ordering::Relaxed);
                    state.device_count.store(0, Ordering::Relaxed);
                    state.clear_controllers();
                    emit_status(&app, &state);
                }
            }
        }
    }
    info!("[joycon] scanner stopped");
}

fn device_loop<R: Runtime>(
    app: AppHandle<R>,
    state: State,
    registry: Arc<ActionRegistry<R>>,
    dev: Arc<Mutex<JoyConDevice>>,
    device_idx: u8,
) {
    state.device_count.fetch_add(1, Ordering::Relaxed);
    // `connected` is driven by the real data stream inside `drive()` (set true
    // only once we actually receive a valid 0x30 report), so a device that is
    // detected but cannot stream input does not falsely show as connected.
    let side = detect_side(&dev);
    let (serial, is_first_pair) = serial_and_first_pair(&app, &state, &dev);
    state.register_controller(ControllerKind::from(side), serial.clone());
    let _ = app.emit(
        "joycon://controller_detected",
        ControllerDetected {
            kind: ControllerKind::from(side),
            serial: serial.clone(),
            device_index: device_idx,
            is_first_pair,
        },
    );
    emit_status(&app, &state);

    // Retry loop: keep trying forever so JC reconnect (after sleep / re-pair)
    // restores the same mapping automatically. Backs off when persistently failing.
    let mut consecutive_failures: u32 = 0;
    while state.running.load(Ordering::Relaxed) {
        match drive(&app, &state, &registry, dev.clone(), device_idx, side) {
            Ok(()) => break,
            Err(e) => {
                if consecutive_failures < 5 || consecutive_failures % 30 == 0 {
                    warn!(
                        "[joycon] device {device_idx} drive ended: {e:?} (retry {consecutive_failures})"
                    );
                }
                consecutive_failures = consecutive_failures.saturating_add(1);
                state.connected.store(false, Ordering::Relaxed);
                emit_status(&app, &state);
                // Do NOT re-emit controller_detected on every drive retry — while
                // Bluetooth is down that fires every ~50ms and spams "connected"
                // toasts in the frontend.
                // Backoff: keep retries snappy so a Bluetooth reconnect recovers
                // quickly (the device may already be back; we don't want to sit
                // in a long sleep). Cap at 400ms even when persistently failing.
                let backoff_ms = match consecutive_failures {
                    0..=10 => 50,
                    11..=25 => 150,
                    _ => 400,
                };
                thread::sleep(Duration::from_millis(backoff_ms));
            }
        }
    }

    state.unregister_controller(&serial);
    let prev = state.device_count.fetch_sub(1, Ordering::Relaxed);
    if prev <= 1 {
        state.connected.store(false, Ordering::Relaxed);
    }
    emit_status(&app, &state);
}

fn detect_side(dev: &Arc<Mutex<JoyConDevice>>) -> JoyConSide {
    if let Ok(d) = dev.lock() {
        match d.device_type() {
            JoyConDeviceType::JoyConL => JoyConSide::Left,
            JoyConDeviceType::JoyConR => JoyConSide::Right,
            JoyConDeviceType::ProCon => JoyConSide::Pro,
        }
    } else {
        JoyConSide::Left
    }
}

fn serial_and_first_pair<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    dev: &Arc<Mutex<JoyConDevice>>,
) -> (String, bool) {
    let serial = dev
        .lock()
        .ok()
        .map(|d| d.serial_number().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let mut first = false;
    if let Ok(mut seen) = state.seen_serials.lock() {
        if !seen.contains(&serial) {
            seen.insert(serial.clone());
            first = true;
            // persist async, ignore failure
            let cfg = crate::settings::PluginConfig {
                enabled: state.enabled.load(Ordering::Relaxed),
                mappings: state.mappings.lock().map(|g| g.clone()).unwrap_or_default(),
                seen_serials: seen.clone(),
                imu: state.imu.lock().map(|g| g.clone()).unwrap_or_default(),
                profiles: state.profiles.lock().map(|g| g.clone()).unwrap_or_default(),
                per_app_enabled: state.per_app_enabled.load(Ordering::Relaxed),
            };
            let _ = save(app, &cfg);
        }
    }
    (serial, first)
}

#[derive(Clone, Copy)]
struct PressFsm {
    pressed_at: Option<Instant>,
    last_release: Option<Instant>,
    long_fired: bool,
    last_repeat_at: Option<Instant>,
}

impl PressFsm {
    fn new() -> Self {
        Self {
            pressed_at: None,
            last_release: None,
            long_fired: false,
            last_repeat_at: None,
        }
    }
}

fn is_transient_read_error(err: &JoyConError) -> bool {
    matches!(
        err,
        JoyConError::JoyConReportError(JoyConReportError::EmptyReport)
    )
}

fn read_report(
    mode: &StandardFullMode<SimpleJoyConDriver>,
) -> Result<StandardInputReport<IMUData>, JoyConError> {
    mode.read_input_report_timeout(READ_TIMEOUT_MS)
}

fn mark_connected_if_standard<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    report: &StandardInputReport<IMUData>,
) {
    if report.common.input_report_id != 0x30 {
        return;
    }
    if !state.connected.swap(true, Ordering::Relaxed) {
        emit_status(app, state);
    }
}

fn reassert_standard_mode(mode: &mut StandardFullMode<SimpleJoyConDriver>, count: u32) {
    if count <= 4 || count.is_power_of_two() {
        let _ = mode
            .driver_mut()
            .send_sub_command(SubCommand::SetInputReportMode, &[0x30u8]);
    }
}

fn drive<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    registry: &Arc<ActionRegistry<R>>,
    dev: Arc<Mutex<JoyConDevice>>,
    device_idx: u8,
    side: JoyConSide,
) -> JoyConResult<()> {
    let driver = SimpleJoyConDriver::new(&dev)?;
    let mut mode_full = StandardFullMode::new(driver)?;
    info!("[joycon] device {device_idx} ({side:?}) opened");

    let mut prev_state: HashMap<JoyConButton, bool> = HashMap::new();
    let mut fsm: HashMap<JoyConButton, PressFsm> = HashMap::new();
    let mut gesture_until: HashMap<JoyConButton, Instant> = HashMap::new();
    let mut last_gesture_at: HashMap<JoyConButton, Instant> = HashMap::new();
    let mut last_status_emit = Instant::now() - BATTERY_EMIT_INTERVAL;
    let mut non_standard_reports: u32 = 0;

    // Settle window after a (re)open. Drain IMU/button reports without
    // dispatching gestures, but still mark connected once 0x30 reports flow.
    let warmup_end = Instant::now() + WARMUP;
    while state.running.load(Ordering::Relaxed) && Instant::now() < warmup_end {
        match read_report(&mode_full) {
            Ok(report) => {
                if report.common.input_report_id != 0x30 {
                    non_standard_reports = non_standard_reports.saturating_add(1);
                    reassert_standard_mode(&mut mode_full, non_standard_reports);
                } else {
                    non_standard_reports = 0;
                    mark_connected_if_standard(app, state, &report);
                }
                if let Ok(mut g) = state.last_seen.lock() {
                    *g = Instant::now();
                }
            }
            Err(e) if is_transient_read_error(&e) => {}
            Err(e) => return Err(e),
        }
        thread::sleep(POLL);
    }

    // After the warm-up drain, give the IMU a longer settle window before
    // acting on motion gestures. Physical buttons & sticks respond immediately.
    let stream_start = Instant::now();
    let gesture_settle = Duration::from_millis(2500);

    while state.running.load(Ordering::Relaxed) {
        let report = match read_report(&mode_full) {
            Ok(r) => r,
            Err(e) if is_transient_read_error(&e) => continue,
            Err(e) => return Err(e),
        };
        if let Ok(mut g) = state.last_seen.lock() {
            *g = Instant::now();
        }

        // Only act on genuine 0x30 reports; re-assert mode aggressively right
        // after reconnect when subcommand replies (0x21) may dominate the stream.
        if report.common.input_report_id != 0x30 {
            non_standard_reports = non_standard_reports.saturating_add(1);
            reassert_standard_mode(&mut mode_full, non_standard_reports);
            continue;
        }
        non_standard_reports = 0;

        mark_connected_if_standard(app, state, &report);

        let now = Instant::now();
        if now.duration_since(stream_start) >= gesture_settle {
            detect_gestures(state, &report, now, &mut gesture_until, &mut last_gesture_at);
        }

        for &btn in ALL_BUTTONS {
            let pressed = is_button_pressed(&report, btn)
                || stick_tilted(&report, btn)
                || stick_direction(&report, btn)
                || gesture_active(btn, now, &gesture_until);
            let was = prev_state.get(&btn).copied().unwrap_or(false);

            if pressed != was {
                handle_edge(
                    app,
                    state,
                    registry,
                    &mut fsm,
                    btn,
                    pressed,
                    now,
                    device_idx,
                    side,
                );
                prev_state.insert(btn, pressed);
            } else if pressed {
                check_long_press(app, state, registry, &mut fsm, btn, now);
            }
        }

        if last_status_emit.elapsed() > BATTERY_EMIT_INTERVAL {
            let pct = battery_to_pct(report.common.battery.level);
            state.battery_pct.store(pct, Ordering::Relaxed);
            state
                .charging
                .store(report.common.battery.is_charging, Ordering::Relaxed);
            emit_status(app, state);
            last_status_emit = Instant::now();
        }

        // No sleep — read_input_report() blocks until next 60Hz report.
        thread::sleep(POLL);
    }
    Ok(())
}

fn handle_edge<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    registry: &Arc<ActionRegistry<R>>,
    fsm: &mut HashMap<JoyConButton, PressFsm>,
    btn: JoyConButton,
    pressed: bool,
    now: Instant,
    device_idx: u8,
    side: JoyConSide,
) {
    let _ = app.emit(
        "joycon://button",
        JoyConButtonEvent {
            button: btn,
            pressed,
            device_index: device_idx,
            side,
        },
    );

    if state.capture_mode.load(Ordering::Relaxed) {
        return;
    }
    if !state.enabled.load(Ordering::Relaxed) {
        return;
    }

    let mapping = lookup_mapping(state, btn);
    let Some((payload, mode)) = mapping else {
        return;
    };

    let entry = fsm.entry(btn).or_insert_with(PressFsm::new);

    match mode {
        TriggerMode::Hold => {
            if pressed {
                entry.pressed_at = Some(now);
                fire(app, registry, &payload, btn, mode, true);
            } else {
                entry.pressed_at = None;
                fire(app, registry, &payload, btn, mode, false);
            }
        }
        TriggerMode::Tap => {
            if pressed {
                entry.pressed_at = Some(now);
            } else if let Some(start) = entry.pressed_at.take() {
                if now.duration_since(start) <= TAP_THRESHOLD {
                    fire(app, registry, &payload, btn, mode, true);
                }
            }
        }
        TriggerMode::DoubleTap => {
            if pressed {
                entry.pressed_at = Some(now);
            } else if let Some(start) = entry.pressed_at.take() {
                if now.duration_since(start) > TAP_THRESHOLD {
                    entry.last_release = None;
                    return;
                }
                if let Some(prev_release) = entry.last_release {
                    if now.duration_since(prev_release) <= DOUBLE_TAP_WINDOW {
                        fire(app, registry, &payload, btn, mode, true);
                        entry.last_release = None;
                        return;
                    }
                }
                entry.last_release = Some(now);
            }
        }
        TriggerMode::LongPress => {
            if pressed {
                entry.pressed_at = Some(now);
                entry.long_fired = false;
            } else {
                entry.pressed_at = None;
                entry.long_fired = false;
            }
        }
        TriggerMode::Repeat => {
            if pressed {
                // First press: fire immediately, start repeat timer.
                entry.pressed_at = Some(now);
                entry.last_repeat_at = Some(now);
                fire(app, registry, &payload, btn, mode, true);
            } else {
                entry.pressed_at = None;
                entry.last_repeat_at = None;
            }
        }
    }
}

fn check_long_press<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    registry: &Arc<ActionRegistry<R>>,
    fsm: &mut HashMap<JoyConButton, PressFsm>,
    btn: JoyConButton,
    now: Instant,
) {
    if state.capture_mode.load(Ordering::Relaxed) || !state.enabled.load(Ordering::Relaxed) {
        return;
    }
    let Some((payload, mode)) = lookup_mapping(state, btn) else {
        return;
    };
    let Some(entry) = fsm.get_mut(&btn) else {
        return;
    };
    let Some(start) = entry.pressed_at else {
        return;
    };

    match mode {
        TriggerMode::LongPress => {
            if entry.long_fired {
                return;
            }
            if now.duration_since(start) >= LONG_PRESS_THRESHOLD {
                fire(app, registry, &payload, btn, mode, true);
                entry.long_fired = true;
            }
        }
        TriggerMode::Repeat => {
            // Wait initial delay (350ms) before starting to repeat,
            // then fire every interval (50ms) while held.
            if now.duration_since(start) < REPEAT_INITIAL_DELAY {
                return;
            }
            let last = entry.last_repeat_at.unwrap_or(start);
            if now.duration_since(last) >= REPEAT_INTERVAL {
                fire(app, registry, &payload, btn, mode, true);
                entry.last_repeat_at = Some(now);
            }
        }
        _ => {}
    }
}

fn lookup_mapping(state: &State, btn: JoyConButton) -> Option<(ActionPayload, TriggerMode)> {
    // Honor per-app profile if active
    let mappings = state.active_mappings();
    let m = mappings.iter().find(|m| m.button == btn)?;
    let payload = m.payload.clone()?;
    Some((payload, m.mode))
}

fn fire<R: Runtime>(
    app: &AppHandle<R>,
    registry: &Arc<ActionRegistry<R>>,
    payload: &ActionPayload,
    button: JoyConButton,
    mode: TriggerMode,
    pressed: bool,
) {
    dispatch(app, registry, payload, button, mode, pressed);
    let action_name = match payload {
        ActionPayload::Builtin { id } => id.clone(),
        ActionPayload::Keyboard { .. } => "<macro>".to_string(),
        ActionPayload::Text { .. } => "<text>".to_string(),
        ActionPayload::OpenApp { bundle_id } => format!("<open:{bundle_id}>"),
        ActionPayload::Shell { .. } => "<shell>".to_string(),
        ActionPayload::AppleScript { .. } => "<applescript>".to_string(),
    };
    let _ = app.emit(
        "joycon://action_fired",
        JoyConActionFired {
            action: action_name,
            button,
            mode,
            pressed,
        },
    );
}

fn battery_to_pct(level: BatteryLevel) -> u8 {
    match level {
        BatteryLevel::Empty => 0,
        BatteryLevel::Critical => 15,
        BatteryLevel::Low => 35,
        BatteryLevel::Medium => 65,
        BatteryLevel::Full => 100,
    }
}

fn is_button_pressed(report: &StandardInputReport<IMUData>, btn: JoyConButton) -> bool {
    let bs = &report.common.pushed_buttons;
    match btn {
        JoyConButton::A => bs.right.contains(&Buttons::A),
        JoyConButton::B => bs.right.contains(&Buttons::B),
        JoyConButton::X => bs.right.contains(&Buttons::X),
        JoyConButton::Y => bs.right.contains(&Buttons::Y),
        JoyConButton::Plus => bs.shared.contains(&Buttons::Plus),
        JoyConButton::Minus => bs.shared.contains(&Buttons::Minus),
        JoyConButton::Home => bs.shared.contains(&Buttons::Home),
        JoyConButton::Capture => bs.shared.contains(&Buttons::Capture),
        JoyConButton::L => bs.left.contains(&Buttons::L),
        JoyConButton::R => bs.right.contains(&Buttons::R),
        JoyConButton::Zl => bs.left.contains(&Buttons::ZL),
        JoyConButton::Zr => bs.right.contains(&Buttons::ZR),
        JoyConButton::LStick => bs.shared.contains(&Buttons::LStick),
        JoyConButton::RStick => bs.shared.contains(&Buttons::RStick),
        JoyConButton::SlLeft => bs.left.contains(&Buttons::SL),
        JoyConButton::SrLeft => bs.left.contains(&Buttons::SR),
        JoyConButton::SlRight => bs.right.contains(&Buttons::SL),
        JoyConButton::SrRight => bs.right.contains(&Buttons::SR),
        JoyConButton::Up => bs.left.contains(&Buttons::Up),
        JoyConButton::Down => bs.left.contains(&Buttons::Down),
        JoyConButton::Left => bs.left.contains(&Buttons::Left),
        JoyConButton::Right => bs.left.contains(&Buttons::Right),
        // Virtual buttons resolved by stick_direction / gesture_active
        JoyConButton::LStickUp
        | JoyConButton::LStickDown
        | JoyConButton::LStickLeft
        | JoyConButton::LStickRight
        | JoyConButton::RStickUp
        | JoyConButton::RStickDown
        | JoyConButton::RStickLeft
        | JoyConButton::RStickRight
        | JoyConButton::Shake
        | JoyConButton::FlipUp
        | JoyConButton::FlipDown
        | JoyConButton::TiltLeft
        | JoyConButton::TiltRight
        | JoyConButton::ShakeHorizontal
        | JoyConButton::ShakeVertical => false,
    }
}

/// True if the analog stick associated with `btn` is tilted past the threshold.
/// Treats LStick / RStick as "pressed" when their analog data drifts from center.
fn stick_tilted(report: &StandardInputReport<IMUData>, btn: JoyConButton) -> bool {
    let stick = match btn {
        JoyConButton::LStick => &report.common.left_analog_stick_data,
        JoyConButton::RStick => &report.common.right_analog_stick_data,
        _ => return false,
    };
    let dx = stick.horizontal as i32 - STICK_CENTER;
    let dy = stick.vertical as i32 - STICK_CENTER;
    dx.abs() > STICK_TILT_THRESHOLD || dy.abs() > STICK_TILT_THRESHOLD
}

/// Virtual directional buttons from analog stick (independent of click).
fn stick_direction(report: &StandardInputReport<IMUData>, btn: JoyConButton) -> bool {
    let (stick, dir): (&_, _) = match btn {
        JoyConButton::LStickUp => (&report.common.left_analog_stick_data, "up"),
        JoyConButton::LStickDown => (&report.common.left_analog_stick_data, "down"),
        JoyConButton::LStickLeft => (&report.common.left_analog_stick_data, "left"),
        JoyConButton::LStickRight => (&report.common.left_analog_stick_data, "right"),
        JoyConButton::RStickUp => (&report.common.right_analog_stick_data, "up"),
        JoyConButton::RStickDown => (&report.common.right_analog_stick_data, "down"),
        JoyConButton::RStickLeft => (&report.common.right_analog_stick_data, "left"),
        JoyConButton::RStickRight => (&report.common.right_analog_stick_data, "right"),
        _ => return false,
    };
    let dx = stick.horizontal as i32 - STICK_CENTER;
    let dy = stick.vertical as i32 - STICK_CENTER;
    match dir {
        "up" => dy > STICK_DIR_THRESHOLD,
        "down" => dy < -STICK_DIR_THRESHOLD,
        "right" => dx > STICK_DIR_THRESHOLD,
        "left" => dx < -STICK_DIR_THRESHOLD,
        _ => false,
    }
}

fn detect_gestures(
    state: &State,
    report: &StandardInputReport<IMUData>,
    now: Instant,
    gesture_until: &mut HashMap<JoyConButton, Instant>,
    last_gesture_at: &mut HashMap<JoyConButton, Instant>,
) {
    let imu = state.imu.lock().map(|g| g.clone()).unwrap_or_default();
    let axis = &report.extra.data[0];
    let accel_mag2 = (axis.accel_x as i32).pow(2)
        + (axis.accel_y as i32).pow(2)
        + (axis.accel_z as i32).pow(2);
    let shake_mag2 = (imu.shake_threshold as i64).pow(2) as i32;
    let cooldown = Duration::from_millis(imu.gesture_cooldown_ms as u64);
    let hold = Duration::from_millis(imu.gesture_hold_ms as u64);
    let trigger = |btn: JoyConButton,
                   gesture_until: &mut HashMap<_, _>,
                   last_gesture_at: &mut HashMap<_, _>| {
        if let Some(&last) = last_gesture_at.get(&btn) {
            if now.duration_since(last) < cooldown {
                return;
            }
        }
        gesture_until.insert(btn, now + hold);
        last_gesture_at.insert(btn, now);
    };
    if accel_mag2 > shake_mag2 {
        trigger(JoyConButton::Shake, gesture_until, last_gesture_at);
    }
    if axis.gyro_2 as i32 > imu.flip_threshold {
        trigger(JoyConButton::FlipUp, gesture_until, last_gesture_at);
    }
    if (axis.gyro_2 as i32) < -imu.flip_threshold {
        trigger(JoyConButton::FlipDown, gesture_until, last_gesture_at);
    }
    if axis.gyro_1 as i32 > imu.flip_threshold {
        trigger(JoyConButton::TiltRight, gesture_until, last_gesture_at);
    }
    if (axis.gyro_1 as i32) < -imu.flip_threshold {
        trigger(JoyConButton::TiltLeft, gesture_until, last_gesture_at);
    }
    let dir_shake = imu.shake_threshold * 6 / 10;
    if (axis.accel_x as i32).abs() > dir_shake {
        trigger(JoyConButton::ShakeHorizontal, gesture_until, last_gesture_at);
    }
    if (axis.accel_y as i32).abs() > dir_shake {
        trigger(JoyConButton::ShakeVertical, gesture_until, last_gesture_at);
    }
}

fn gesture_active(
    btn: JoyConButton,
    now: Instant,
    gesture_until: &HashMap<JoyConButton, Instant>,
) -> bool {
    matches!(
        btn,
        JoyConButton::Shake
            | JoyConButton::FlipUp
            | JoyConButton::FlipDown
            | JoyConButton::TiltLeft
            | JoyConButton::TiltRight
            | JoyConButton::ShakeHorizontal
            | JoyConButton::ShakeVertical
    ) && gesture_until.get(&btn).map_or(false, |&t| now < t)
}

fn emit_status<R: Runtime>(app: &AppHandle<R>, state: &State) {
    let s: JoyConStatus = state.snapshot();
    let _ = app.emit("joycon://status", s);
}

#[cfg(target_os = "macos")]
pub fn spawn_frontmost_watcher<R: Runtime>(app: AppHandle<R>, state: State) {
    thread::spawn(move || {
        let mut last: Option<String> = None;
        while state.running.load(Ordering::Relaxed) {
            let bundle = read_frontmost_bundle();
            if bundle != last {
                if let Ok(mut g) = state.frontmost_bundle.lock() {
                    *g = bundle.clone();
                }
                let _ = app.emit("joycon://frontmost_changed", bundle.clone());
                last = bundle;
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}

#[cfg(target_os = "macos")]
fn read_frontmost_bundle() -> Option<String> {
    let out = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get bundle identifier of first process whose frontmost is true",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(target_os = "macos")]
pub fn read_frontmost_app() -> Option<(String, String)> {
    let out = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to set p to first process whose frontmost is true\nset n to name of p\nset b to bundle identifier of p\nreturn n & \"|\" & b",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let mut parts = s.splitn(2, '|');
    let name = parts.next()?.to_string();
    let bundle = parts.next().unwrap_or("").to_string();
    Some((name, bundle))
}

#[cfg(not(target_os = "macos"))]
pub fn read_frontmost_app() -> Option<(String, String)> {
    None
}
