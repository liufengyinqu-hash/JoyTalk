//! Joy-Con scanner + per-device input loop with payload dispatch and ControllerDetected emit.

use joycon_rs::prelude::input_report_mode::standard_full_mode::IMUData;
use joycon_rs::prelude::input_report_mode::{BatteryLevel, StandardInputReport};
use joycon_rs::prelude::*;
use joycon_rs::joycon::ir::{EnableProgress as IrEnableProgress, IrSession};
use joycon_rs::joycon::nfc::{
    coalesce_nfc_samples, peek_nfc_report, EnableProgress as NfcEnableProgress, NfcSession,
};
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
    ActionPayload, ButtonMapping, ControllerDetected, ControllerKind, IrLiveSample, McuMode,
    McuRuntime, McuStatus, NfcLiveSample, JoyConActionFired, JoyConButton, JoyConButtonEvent,
    JoyConSide, JoyConStatus, TriggerMode,
};

const POLL: Duration = Duration::from_millis(8);
/// Main input read: ~1.5 frames at 60 Hz — low latency without busy-spinning.
const READ_TIMEOUT_MS: i32 = 25;
/// Drain already-buffered HID reports without blocking the input loop.
const READ_DRAIN_TIMEOUT_MS: i32 = 0;
const MAX_NFC_DRAIN_READS: u32 = 2;
/// Idle NFC MCU poll (~5 Hz) — keeps input latency low on the shared HID link.
const NFC_POLL_IDLE: Duration = Duration::from_millis(200);
/// Faster poll while a tag read is in progress (~10 Hz, only when buttons idle).
const NFC_POLL_READING: Duration = Duration::from_millis(100);
/// Defer NFC OUT traffic briefly after stick/button activity.
const NFC_INPUT_QUIET: Duration = Duration::from_millis(150);
const STATUS_TIMEOUT: Duration = Duration::from_secs(8);
const BATTERY_EMIT_INTERVAL: Duration = Duration::from_secs(10);
const IR_EMIT_INTERVAL: Duration = Duration::from_millis(100);
const NFC_EMIT_INTERVAL: Duration = Duration::from_millis(100);
const NFC_DIAG_INTERVAL: Duration = Duration::from_secs(2);
const MCU_ENABLE_DEFER: Duration = Duration::from_millis(150);
const MCU_ENABLE_SLICE: Duration = Duration::from_millis(150);

enum PendingMcuEnable {
    Ir(IrSession),
    Nfc(NfcSession),
}

struct McuSwitchJob {
    desired: McuMode,
    pending: PendingMcuEnable,
}

enum AdvanceMcuResult {
    Continue,
    Done,
    Failed,
}
const WARMUP: Duration = Duration::from_millis(300);
const STICK_CALIBRATION: Duration = Duration::from_millis(500);

/// Build UI sample from a coalesced NFC tick sample.
fn nfc_live_from_sample(sample: &joycon_rs::joycon::nfc::NfcSample) -> NfcLiveSample {
    NfcLiveSample {
        session_active: true,
        tag_present: sample.tag_present,
        tag_detected: sample.tag_detected,
        uid: if sample.tag_present {
            sample.uid_hex()
        } else {
            String::new()
        },
        uid_len: if sample.tag_present {
            sample.uid.len() as u8
        } else {
            0
        },
        tag_type: if sample.tag_present { sample.tag_type } else { 0 },
        nfc_state: sample.state,
    }
}

fn apply_nfc_sample_update(
    sample: &joycon_rs::joycon::nfc::NfcSample,
    last_logged_nfc_uid: &mut String,
    last_nfc_tag_present: &mut bool,
) -> NfcLiveSample {
    let live = nfc_live_from_sample(sample);
    if sample.tag_present && !sample.uid.is_empty() {
        let uid = sample.uid_hex();
        if uid != *last_logged_nfc_uid {
            info!(
                "[joycon] NFC tag detected uid={uid} type=0x{:02x}",
                sample.tag_type
            );
            *last_logged_nfc_uid = uid;
        }
    } else if *last_nfc_tag_present
        && !sample.tag_present
        && joycon_rs::joycon::nfc::nfc_state_ready(sample.state)
    {
        info!("[joycon] NFC tag removed");
        last_logged_nfc_uid.clear();
    }
    *last_nfc_tag_present = sample.tag_present;
    live
}

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
    JoyConButton::A,
    JoyConButton::B,
    JoyConButton::X,
    JoyConButton::Y,
    JoyConButton::Plus,
    JoyConButton::Minus,
    JoyConButton::Home,
    JoyConButton::Capture,
    JoyConButton::L,
    JoyConButton::R,
    JoyConButton::Zl,
    JoyConButton::Zr,
    JoyConButton::LStick,
    JoyConButton::RStick,
    JoyConButton::SlLeft,
    JoyConButton::SrLeft,
    JoyConButton::SlRight,
    JoyConButton::SrRight,
    JoyConButton::Up,
    JoyConButton::Down,
    JoyConButton::Left,
    JoyConButton::Right,
    JoyConButton::LStickUp,
    JoyConButton::LStickDown,
    JoyConButton::LStickLeft,
    JoyConButton::LStickRight,
    JoyConButton::RStickUp,
    JoyConButton::RStickDown,
    JoyConButton::RStickLeft,
    JoyConButton::RStickRight,
    JoyConButton::Shake,
    JoyConButton::FlipUp,
    JoyConButton::FlipDown,
    JoyConButton::TiltLeft,
    JoyConButton::TiltRight,
    JoyConButton::ShakeHorizontal,
    JoyConButton::ShakeVertical,
    JoyConButton::IrProximity,
    JoyConButton::NfcTagPresent,
];

// Mapping/repeat thresholds apply to calibrated delta (resting offset already subtracted).
const STICK_DIR_PRESS_THRESHOLD: i32 = 750;
const STICK_DIR_RELEASE_THRESHOLD: i32 = 500;

/// Per-session stick center sampled while the controller rests at connect time.
#[derive(Clone, Copy, Debug)]
struct StickCalibration {
    left_h_center: i32,
    left_v_center: i32,
    right_h_center: i32,
    right_v_center: i32,
}

impl StickCalibration {
    fn nominal() -> Self {
        Self {
            left_h_center: STICK_CENTER,
            left_v_center: STICK_CENTER,
            right_h_center: STICK_CENTER,
            right_v_center: STICK_CENTER,
        }
    }
}

pub fn spawn_scanner<R: Runtime>(
    app: AppHandle<R>,
    state: State,
    registry: Arc<ActionRegistry<R>>,
) {
    thread::spawn(move || scanner_loop(app, state, registry));
}

fn scanner_loop<R: Runtime>(app: AppHandle<R>, state: State, registry: Arc<ActionRegistry<R>>) {
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
                let devices_active = state.device_count.load(Ordering::Relaxed) > 0;
                if state.connected.load(Ordering::Relaxed)
                    && elapsed > STATUS_TIMEOUT
                    && !devices_active
                {
                    state.connected.store(false, Ordering::Relaxed);
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
    let kind = ControllerKind::from(side);
    // Do not add to connected_controllers yet — wait until drive() receives 0x30.
    let _ = app.emit(
        "joycon://controller_detected",
        ControllerDetected {
            kind,
            serial: serial.clone(),
            device_index: device_idx,
            is_first_pair,
        },
    );

    // Retry loop: keep trying forever so JC reconnect (after sleep / re-pair)
    // restores the same mapping automatically. Backs off when persistently failing.
    let mut consecutive_failures: u32 = 0;
    while state.running.load(Ordering::Relaxed) {
        match drive(
            &app,
            &state,
            &registry,
            dev.clone(),
            device_idx,
            side,
            kind,
            &serial,
        ) {
            Ok(()) => break,
            Err(e) => {
                if consecutive_failures < 5 || consecutive_failures % 30 == 0 {
                    warn!(
                        "[joycon] device {device_idx} ({serial}) drive ended: {e:?} (retry {consecutive_failures})"
                    );
                }
                consecutive_failures = consecutive_failures.saturating_add(1);
                // Keep the controller registered while retrying — it is still paired
                // over Bluetooth; unregister only when device_loop exits for good.
                state.connected.store(
                    state
                        .connected_controllers
                        .lock()
                        .map(|g| !g.is_empty())
                        .unwrap_or(false),
                    Ordering::Relaxed,
                );
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
    let d = dev.lock().unwrap_or_else(|e| e.into_inner());
    let dt = d.device_type();
    let side = match dt {
        JoyConDeviceType::JoyConL => JoyConSide::Left,
        JoyConDeviceType::JoyConR => JoyConSide::Right,
        JoyConDeviceType::ProCon => JoyConSide::Pro,
    };
    info!("[joycon] detected side: {side:?} (device_type={dt:?}, serial={})", d.serial_number());
    side
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
    let mut seen = state.seen_serials.lock().unwrap_or_else(|e| e.into_inner());
    {
        if !seen.contains(&serial) {
            seen.insert(serial.clone());
            first = true;
            // persist async, ignore failure
            let cfg = crate::settings::PluginConfig {
                config_version: crate::settings::current_config_version(),
                enabled: state.enabled.load(Ordering::Relaxed),
                mappings: state.mappings.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                seen_serials: seen.clone(),
                imu: state.imu.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                mcu: state.mcu.lock().unwrap_or_else(|e| e.into_inner()).clone(),
                profiles: state.profiles.lock().unwrap_or_else(|e| e.into_inner()).clone(),
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

/// Sub-command NACK during warm-up / calibrate (MCU busy, empty payload) — retry in-loop.
fn is_transient_setup_error(err: &JoyConError) -> bool {
    if is_transient_read_error(err) {
        return true;
    }
    matches!(err, JoyConError::SubCommandError(_, data) if data.is_empty())
}

fn read_report(
    mode: &StandardFullMode<SimpleJoyConDriver>,
) -> Result<StandardInputReport<IMUData>, JoyConError> {
    mode.read_input_report_timeout(READ_TIMEOUT_MS)
}

fn read_report_raw(
    mode: &StandardFullMode<SimpleJoyConDriver>,
) -> Result<(StandardInputReport<IMUData>, [u8; 362]), JoyConError> {
    read_report_raw_timeout(mode, READ_TIMEOUT_MS)
}

fn read_report_raw_timeout(
    mode: &StandardFullMode<SimpleJoyConDriver>,
    timeout_ms: i32,
) -> Result<(StandardInputReport<IMUData>, [u8; 362]), JoyConError> {
    let mut buf = [0u8; 362];
    mode.driver()
        .read_timeout(&mut buf, timeout_ms)?;
    let report = StandardInputReport::try_from(buf)?;
    Ok((report, buf))
}

fn is_standard_stream_report(id: u8) -> bool {
    id == 0x30 || id == 0x31
}

fn nfc_poll_interval(state: &State) -> Duration {
    state
        .nfc_sample
        .lock()
        .map(|g| {
            if g.tag_detected && !g.tag_present {
                NFC_POLL_READING
            } else {
                NFC_POLL_IDLE
            }
        })
        .unwrap_or(NFC_POLL_IDLE)
}

fn frame_has_input(
    report: &StandardInputReport<IMUData>,
    stick_cal: &StickCalibration,
    side: JoyConSide,
) -> bool {
    ALL_BUTTONS.iter().any(|&btn| {
        if !button_relevant_for_side(btn, side) {
            return false;
        }
        is_button_pressed(report, btn)
            || stick_tilted(report, btn, stick_cal)
            || stick_direction(report, btn, stick_cal, false)
    })
}

fn mark_connected_if_standard<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    report: &StandardInputReport<IMUData>,
    kind: ControllerKind,
    serial: &str,
) {
    if !is_standard_stream_report(report.common.input_report_id) {
        return;
    }
    state.register_controller(kind, serial.to_string());
    if !state.connected.swap(true, Ordering::Relaxed) {
        emit_status(app, state);
    }
}

fn desired_mcu_mode(state: &State, side: JoyConSide) -> McuMode {
    if !matches!(side, JoyConSide::Right) {
        return McuMode::Off;
    }
    state
        .mcu
        .lock()
        .map(|g| g.mode)
        .unwrap_or(McuMode::Off)
}

fn session_mcu_mode(ir_session: &Option<IrSession>, nfc_session: &Option<NfcSession>) -> McuMode {
    if nfc_session
        .as_ref()
        .is_some_and(|s| s.is_active())
    {
        McuMode::Nfc
    } else if ir_session.as_ref().is_some_and(|s| s.is_active()) {
        McuMode::Ir
    } else {
        McuMode::Off
    }
}

fn teardown_mcu_sessions(
    ir_session: &mut Option<IrSession>,
    nfc_session: &mut Option<NfcSession>,
    driver: &mut SimpleJoyConDriver,
) {
    let had_mcu = ir_session.as_ref().is_some_and(|s| s.is_active())
        || nfc_session.as_ref().is_some_and(|s| s.is_active());
    if let Some(mut ir) = ir_session.take() {
        let _ = ir.disable(driver);
    }
    if let Some(mut nfc) = nfc_session.take() {
        let _ = nfc.disable(driver);
    }
    if had_mcu {
        // Reset MCU when switching modes so the next IR/NFC enable starts clean.
        let _ = driver.send_sub_command(SubCommand::ResetNFC_IR_MCU, &[]);
        let _ = driver.send_sub_command(SubCommand::SetInputReportMode, &[0x30]);
        thread::sleep(Duration::from_millis(80));
    }
}

fn begin_mcu_switch<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    serial: &str,
    desired: McuMode,
    mode_full: &mut StandardFullMode<SimpleJoyConDriver>,
    ir_session: &mut Option<IrSession>,
    nfc_session: &mut Option<NfcSession>,
) -> Option<McuSwitchJob> {
    let running_before = session_mcu_mode(ir_session, nfc_session);
    if desired == running_before {
        return None;
    }

    state.set_mcu_runtime(McuRuntime {
        active_mode: running_before,
        switching: true,
    });
    emit_mcu_status(app, state);

    if running_before != McuMode::Off && (desired == McuMode::Off || running_before != desired) {
        teardown_mcu_sessions(ir_session, nfc_session, mode_full.driver_mut());
        state.set_ir_sample(IrLiveSample::default());
        state.set_nfc_sample(NfcLiveSample::default());
    }

    state.set_mcu_runtime(McuRuntime {
        active_mode: running_before,
        switching: false,
    });
    emit_mcu_status(app, state);

    if desired == McuMode::Off {
        state.set_mcu_runtime(McuRuntime {
            active_mode: McuMode::Off,
            switching: false,
        });
        emit_mcu_status(app, state);
        return None;
    }

    if running_before == McuMode::Off {
        let _ = mode_full
            .driver_mut()
            .send_sub_command(SubCommand::ResetNFC_IR_MCU, &[]);
        thread::sleep(Duration::from_millis(80));
        let _ = mode_full
            .driver_mut()
            .send_sub_command(SubCommand::SetInputReportMode, &[0x30]);
    }

    let pending = match desired {
        McuMode::Ir => {
            let mut session = IrSession::new();
            session.begin_enable();
            PendingMcuEnable::Ir(session)
        }
        McuMode::Nfc => {
            let mut session = NfcSession::new();
            session.begin_enable();
            PendingMcuEnable::Nfc(session)
        }
        McuMode::Off => return None,
    };

    info!("[joycon] MCU enable started {running_before:?} -> {desired:?} ({serial})");
    Some(McuSwitchJob { desired, pending })
}

fn advance_mcu_switch<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    serial: &str,
    job: &mut McuSwitchJob,
    mode_full: &mut StandardFullMode<SimpleJoyConDriver>,
    ir_session: &mut Option<IrSession>,
    nfc_session: &mut Option<NfcSession>,
) -> AdvanceMcuResult {
    let driver = mode_full.driver_mut();
    let result = match &mut job.pending {
        PendingMcuEnable::Ir(session) => session
            .advance_enable(driver, MCU_ENABLE_SLICE)
            .map(|p| p == IrEnableProgress::Done),
        PendingMcuEnable::Nfc(session) => session
            .advance_enable(driver, MCU_ENABLE_SLICE)
            .map(|p| p == NfcEnableProgress::Done),
    };

    match result {
        Ok(true) => {
            match job.pending {
                PendingMcuEnable::Ir(ref mut session) => {
                    info!("[joycon] IR proximity enabled ({serial})");
                    ir_session.replace(std::mem::replace(session, IrSession::new()));
                    state.set_ir_sample(IrLiveSample {
                        session_active: true,
                        ..Default::default()
                    });
                }
                PendingMcuEnable::Nfc(ref mut session) => {
                    info!("[joycon] NFC enabled ({serial})");
                    nfc_session.replace(std::mem::replace(session, NfcSession::new()));
                    let _ = mode_full
                        .driver_mut()
                        .send_sub_command(SubCommand::SetInputReportMode, &[0x31]);
                    if let Some(ref nfc) = nfc_session {
                        if let Err(e) = nfc.tick_poll(mode_full.driver_mut()) {
                            warn!("[joycon] NFC post-enable poll failed ({serial}): {e:?}");
                        }
                    }
                    let live = NfcLiveSample {
                        session_active: true,
                        nfc_state: 0x01,
                        ..Default::default()
                    };
                    state.set_nfc_sample(live.clone());
                    let _ = app.emit("joycon://nfc_sample", &live);
                }
            }
            state.set_mcu_runtime(McuRuntime {
                active_mode: job.desired,
                switching: false,
            });
            emit_mcu_status(app, state);
            AdvanceMcuResult::Done
        }
        Ok(false) => AdvanceMcuResult::Continue,
        Err(e) => {
            warn!("[joycon] MCU enable failed ({serial}): {e:?}");
            state.set_mcu_runtime(McuRuntime {
                active_mode: McuMode::Off,
                switching: false,
            });
            emit_mcu_status(app, state);
            AdvanceMcuResult::Failed
        }
    }
}

fn emit_mcu_status<R: Runtime>(app: &AppHandle<R>, state: &State) {
    let _ = app.emit("joycon://mcu_status", state.mcu_status_snapshot());
}

fn reassert_standard_mode(
    mode: &mut StandardFullMode<SimpleJoyConDriver>,
    count: u32,
    mcu_stream: bool,
) {
    if count <= 4 || count.is_power_of_two() {
        let report_id = if mcu_stream { 0x31u8 } else { 0x30u8 };
        let _ = mode
            .driver_mut()
            .send_sub_command(SubCommand::SetInputReportMode, &[report_id]);
    }
}

fn drive<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    registry: &Arc<ActionRegistry<R>>,
    dev: Arc<Mutex<JoyConDevice>>,
    device_idx: u8,
    side: JoyConSide,
    kind: ControllerKind,
    serial: &str,
) -> JoyConResult<()> {
    let driver = SimpleJoyConDriver::new(&dev)?;
    let mut mode_full = StandardFullMode::new(driver)?;
    info!("[joycon] device {device_idx} ({side:?}) opened");

    // Enable vibration for rumble feedback
    let _ = mode_full.driver_mut().enable_feature(joycon_features::JoyConFeature::Vibration);

    let mut prev_state: HashMap<JoyConButton, bool> = HashMap::new();
    let mut fsm: HashMap<JoyConButton, PressFsm> = HashMap::new();
    let mut gesture_until: HashMap<JoyConButton, Instant> = HashMap::new();
    let mut last_gesture_at: HashMap<JoyConButton, Instant> = HashMap::new();
    let mut last_status_emit = Instant::now() - BATTERY_EMIT_INTERVAL;
    let mut non_standard_reports: u32 = 0;
    let mut current_led_pattern: u8 = 0;

    // Settle window after a (re)open. Drain IMU/button reports without
    // dispatching gestures, but still mark connected once 0x30 reports flow.
    let warmup_end = Instant::now() + WARMUP;
    while state.running.load(Ordering::Relaxed) && Instant::now() < warmup_end {
        match read_report(&mode_full) {
            Ok(report) => {
                if is_standard_stream_report(report.common.input_report_id) {
                    non_standard_reports = 0;
                    mark_connected_if_standard(app, state, &report, kind, serial);
                } else {
                    non_standard_reports = non_standard_reports.saturating_add(1);
                    reassert_standard_mode(&mut mode_full, non_standard_reports, false);
                }
                *state.last_seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            }
            Err(e) if is_transient_setup_error(&e) => {}
            Err(e) => return Err(e),
        }
        thread::sleep(POLL);
    }

    // Step 1: sample resting stick position before accepting directional input.
    let stick_cal = calibrate_sticks(&mut mode_full, app, state, kind, serial, side)?;

    let mut ir_session: Option<IrSession> = None;
    let mut nfc_session: Option<NfcSession> = None;
    let mut mcu_switch: Option<McuSwitchJob> = None;
    let mut mcu_stream = false;
    let mut last_mcu_fail: Option<(Instant, McuMode)> = None;
    const MCU_IR_RETRY_BACKOFF: Duration = Duration::from_secs(10);
    const MCU_NFC_RETRY_BACKOFF: Duration = Duration::from_secs(15);

    fn mcu_retry_backoff(mode: McuMode) -> Duration {
        match mode {
            McuMode::Nfc => MCU_NFC_RETRY_BACKOFF,
            McuMode::Ir => MCU_IR_RETRY_BACKOFF,
            McuMode::Off => Duration::from_secs(2),
        }
    }

    // After the warm-up drain, give the IMU a longer settle window before
    // acting on motion gestures. Physical buttons & sticks respond immediately.
    let stream_start = Instant::now();
    let gesture_settle = Duration::from_millis(2500);
    let mut last_ir_emit = Instant::now() - IR_EMIT_INTERVAL;
    let mut last_nfc_emit = Instant::now() - NFC_EMIT_INTERVAL;
    let mut last_logged_nfc_uid = String::new();
    let mut last_nfc_ui_present = false;
    let mut last_nfc_tag_present = false;
    let mut last_nfc_state: u8 = 0;
    let mut last_nfc_diag = Instant::now() - NFC_DIAG_INTERVAL;
    let mut last_nfc_poll = Instant::now() - NFC_POLL_IDLE;
    let mut last_input_activity = Instant::now() - NFC_INPUT_QUIET;
    let mut rumble_until: Option<Instant> = None;

    // Set initial LED pattern (idle = LED0 solid)
    apply_led_pattern(mode_full.driver_mut(), 1);
    current_led_pattern = 1;

    while state.running.load(Ordering::Relaxed) {
        let desired_mcu = desired_mcu_mode(state, side);
        let running_mcu = session_mcu_mode(&ir_session, &nfc_session);
        mcu_stream = running_mcu != McuMode::Off || mcu_switch.is_some();

        let ir_threshold = state
            .mcu
            .lock()
            .map(|g| g.white_pixel_threshold)
            .unwrap_or(50);

        if state.nfc_rescan.swap(false, Ordering::Relaxed) {
            if let Some(ref mut nfc) = nfc_session {
                let _ = nfc.restart_scan(mode_full.driver_mut());
                let cleared = NfcLiveSample {
                    session_active: true,
                    nfc_state: 0x01,
                    ..Default::default()
                };
                state.set_nfc_sample(cleared.clone());
                let _ = app.emit("joycon://nfc_sample", &cleared);
                last_logged_nfc_uid.clear();
                last_nfc_tag_present = false;
                last_nfc_ui_present = false;
                last_nfc_state = 0x01;
            }
            last_nfc_poll = Instant::now() - NFC_POLL_IDLE;
        }

        let (report, raw) = match read_report_raw(&mode_full) {
            Ok(r) => r,
            Err(e) if is_transient_read_error(&e) => {
                thread::sleep(POLL);
                continue;
            }
            Err(e) => {
                teardown_mcu_sessions(
                    &mut ir_session,
                    &mut nfc_session,
                    mode_full.driver_mut(),
                );
                state.set_mcu_runtime(McuRuntime::default());
                state.set_ir_sample(IrLiveSample::default());
                state.set_nfc_sample(NfcLiveSample::default());
                return Err(e);
            }
        };

        if !is_standard_stream_report(report.common.input_report_id) {
            non_standard_reports = non_standard_reports.saturating_add(1);
            reassert_standard_mode(&mut mode_full, non_standard_reports, mcu_stream);
            continue;
        }

        let mut frame_reports = Vec::with_capacity(1 + MAX_NFC_DRAIN_READS as usize);
        frame_reports.push((report, raw));

        let now = Instant::now();
        let nfc_poll_due = nfc_session.is_some()
            && now.duration_since(last_input_activity) >= NFC_INPUT_QUIET
            && now.duration_since(last_nfc_poll) >= nfc_poll_interval(state);

        if nfc_poll_due {
            if mcu_stream {
                for _ in 0..MAX_NFC_DRAIN_READS {
                    match read_report_raw_timeout(&mode_full, READ_DRAIN_TIMEOUT_MS) {
                        Ok((extra, extra_raw))
                            if is_standard_stream_report(extra.common.input_report_id) =>
                        {
                            frame_reports.push((extra, extra_raw));
                        }
                        Ok(_) => break,
                        Err(e) if is_transient_read_error(&e) => break,
                        Err(_) => break,
                    }
                }
            }
            if let Some(ref nfc) = nfc_session {
                if let Err(e) = nfc.tick_poll(mode_full.driver_mut()) {
                    warn!("[joycon] NFC tick_poll failed ({serial}): {e:?}");
                }
            }
            last_nfc_poll = now;
        }

        *state.last_seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        non_standard_reports = 0;

        mark_connected_if_standard(app, state, &frame_reports[0].0, kind, serial);

        let main_report = &frame_reports[0].0;
        if frame_has_input(main_report, &stick_cal, side) {
            last_input_activity = now;
        }

        let mut ir_near = false;
        let mut nfc_present = state
            .nfc_sample
            .lock()
            .map(|g| g.tag_present)
            .unwrap_or(false);
        let mut latest_ir: Option<IrLiveSample> = None;
        let mut latest_nfc: Option<NfcLiveSample> = None;
        if let Some(ref mut ir) = ir_session {
            if let Some((_, raw)) = frame_reports.last() {
                if let Ok(Some(sample)) = ir.process_raw_report(mode_full.driver_mut(), raw) {
                    ir_near = sample.proximity_detected(ir_threshold);
                    latest_ir = Some(IrLiveSample {
                        session_active: true,
                        average_intensity: sample.average_intensity,
                        white_pixel_count: sample.white_pixel_count,
                        ambient_noise_count: sample.ambient_noise_count,
                        proximity_active: ir_near,
                    });
                }
            }
        }
        if let Some(ref mut nfc) = nfc_session {
            if nfc_poll_due {
                let mut batch = Vec::with_capacity(frame_reports.len());
                for (_, raw) in &frame_reports {
                    if let Some(sample) = NfcSession::parse_from_raw(raw) {
                        batch.push(sample);
                    }
                }
                if now.duration_since(last_nfc_diag) >= NFC_DIAG_INTERVAL {
                    let (_, diag_raw) = frame_reports.last().expect("frame_reports non-empty");
                    let (hid_id, mcu_id, mcu_state, has_tag, uid_len) = peek_nfc_report(diag_raw);
                    let merged_summary = batch.last().map(|s| {
                        format!(
                            "state=0x{:02x} present={} detected={} uid_len={}",
                            s.state,
                            s.tag_present,
                            s.tag_detected,
                            s.uid.len()
                        )
                    });
                    info!(
                        "[joycon] NFC diag ({serial}): hid=0x{hid_id:02x} mcu=0x{mcu_id:02x} \
                         raw_state=0x{mcu_state:02x} has_tag={has_tag} uid_len={uid_len} \
                         batch={} merged={}",
                        batch.len(),
                        merged_summary.as_deref().unwrap_or("-")
                    );
                    last_nfc_diag = now;
                }
                if let Some(merged) = coalesce_nfc_samples(&batch) {
                    if let Ok(sample) = nfc.apply_sample(mode_full.driver_mut(), merged) {
                        nfc_present = sample.tag_present;
                        latest_nfc = Some(apply_nfc_sample_update(
                            &sample,
                            &mut last_logged_nfc_uid,
                            &mut last_nfc_tag_present,
                        ));
                    }
                }
            }
        }

        if let Some(live) = latest_ir {
            state.set_ir_sample(live.clone());
            log::debug!(
                "[joycon] IR sample: intensity={} white={} ambient={} near={}",
                live.average_intensity,
                live.white_pixel_count,
                live.ambient_noise_count,
                live.proximity_active,
            );
            if now.duration_since(last_ir_emit) >= IR_EMIT_INTERVAL {
                let _ = app.emit("joycon://ir_sample", &live);
                last_ir_emit = now;
            }
        }
        if let Some(live) = latest_nfc {
            state.set_nfc_sample(live.clone());
            log::debug!(
                "[joycon] NFC sample: state={} present={} uid={} type={}",
                live.nfc_state,
                live.tag_present,
                live.uid,
                live.tag_type,
            );
            let present_changed = live.tag_present != last_nfc_ui_present;
            let state_changed = live.nfc_state != last_nfc_state;
            last_nfc_ui_present = live.tag_present;
            last_nfc_state = live.nfc_state;
            let emit_now = present_changed
                || state_changed
                || !live.tag_present
                || now.duration_since(last_nfc_emit) >= NFC_EMIT_INTERVAL;
            if emit_now {
                let _ = app.emit("joycon://nfc_sample", &live);
                last_nfc_emit = now;
            }
        }

        for (frame_report, _) in &frame_reports {
            for &btn in ALL_BUTTONS {
                if !button_relevant_for_side(btn, side) {
                    continue;
                }
                let was = prev_state.get(&btn).copied().unwrap_or(false);
                let pressed = is_button_pressed(frame_report, btn)
                    || stick_tilted(frame_report, btn, &stick_cal)
                    || stick_direction(frame_report, btn, &stick_cal, was)
                    || gesture_active(btn, now, &gesture_until)
                    || (btn == JoyConButton::IrProximity && ir_near)
                    || (btn == JoyConButton::NfcTagPresent && nfc_present);

                if pressed != was {
                    handle_edge(
                        app, state, registry, &mut fsm, btn, pressed, now, device_idx, side,
                    );
                    prev_state.insert(btn, pressed);
                } else if pressed {
                    check_long_press(app, state, registry, &mut fsm, btn, now);
                }
            }
            // Low-frequency NFC: only the primary input frame drives buttons.
            break;
        }

        if now.duration_since(stream_start) >= gesture_settle {
            if let Some((frame_report, _)) = frame_reports.last() {
                detect_gestures(
                    state,
                    frame_report,
                    now,
                    &mut gesture_until,
                    &mut last_gesture_at,
                );
            }
        }

        if let Some((frame_report, _)) = frame_reports.last() {
            if last_status_emit.elapsed() > BATTERY_EMIT_INTERVAL {
                let pct = battery_to_pct(frame_report.common.battery.level);
                state.battery_pct.store(pct, Ordering::Relaxed);
                state
                    .charging
                    .store(frame_report.common.battery.is_charging, Ordering::Relaxed);
                emit_status(app, state);
                last_status_emit = Instant::now();

                // Update LED pattern based on state
                let desired_led = state.led_pattern.load(Ordering::Relaxed);
                if desired_led != current_led_pattern {
                    apply_led_pattern(mode_full.driver_mut(), desired_led);
                    current_led_pattern = desired_led;
                }
            }
        }

        // Process rumble requests
        if state.rumble_pending.swap(false, Ordering::Relaxed) {
            rumble_pulse(mode_full.driver_mut());
            rumble_until = Some(Instant::now() + Duration::from_millis(80));
        }
        if let Some(end) = rumble_until {
            if Instant::now() >= end {
                rumble_stop(mode_full.driver_mut());
                rumble_until = None;
            }
        }

        if matches!(side, JoyConSide::Right) {
            if let Some(ref mut job) = mcu_switch {
                *state.last_seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                let desired_on_fail = job.desired;
                match advance_mcu_switch(
                    app,
                    state,
                    serial,
                    job,
                    &mut mode_full,
                    &mut ir_session,
                    &mut nfc_session,
                ) {
                    AdvanceMcuResult::Done => {
                        mcu_switch = None;
                        last_mcu_fail = None;
                        mcu_stream = session_mcu_mode(&ir_session, &nfc_session) != McuMode::Off;
                    }
                    AdvanceMcuResult::Failed => {
                        mcu_switch = None;
                        if desired_on_fail != McuMode::Off {
                            last_mcu_fail = Some((Instant::now(), desired_on_fail));
                        }
                    }
                    AdvanceMcuResult::Continue => {}
                }
            } else if now.duration_since(stream_start) >= MCU_ENABLE_DEFER
                && desired_mcu != running_mcu
            {
                let backoff = last_mcu_fail
                    .filter(|(_, mode)| *mode == desired_mcu)
                    .map(|(t, _)| t.elapsed() < mcu_retry_backoff(desired_mcu))
                    .unwrap_or(false);
                if !backoff {
                    if last_mcu_fail
                        .filter(|(_, mode)| *mode == desired_mcu)
                        .is_some()
                    {
                        info!(
                            "[joycon] MCU enable retry {running_mcu:?} -> {desired_mcu:?} ({serial})"
                        );
                    } else {
                        info!(
                            "[joycon] MCU hot-switch {running_mcu:?} -> {desired_mcu:?} ({serial})"
                        );
                    }
                    *state.last_seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
                    mcu_switch = begin_mcu_switch(
                        app,
                        state,
                        serial,
                        desired_mcu,
                        &mut mode_full,
                        &mut ir_session,
                        &mut nfc_session,
                    );
                }
            }
        }
    }
    if matches!(side, JoyConSide::Right) {
        teardown_mcu_sessions(
            &mut ir_session,
            &mut nfc_session,
            mode_full.driver_mut(),
        );
        state.set_mcu_runtime(McuRuntime::default());
        emit_mcu_status(app, state);
    }
    state.set_ir_sample(IrLiveSample::default());
    state.set_nfc_sample(NfcLiveSample::default());
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

    let mappings = lookup_mappings_for_button(state, btn);
    if mappings.is_empty() {
        return;
    }

    let entry = fsm.entry(btn).or_insert_with(PressFsm::new);

    let release_duration = if pressed {
        entry.pressed_at = Some(now);
        None
    } else {
        entry
            .pressed_at
            .take()
            .map(|start| now.duration_since(start))
    };

    for (payload, mode) in mappings {
        match mode {
            TriggerMode::Hold => {
                if pressed {
                    fire(app, registry, &payload, btn, mode, true, state);
                } else {
                    fire(app, registry, &payload, btn, mode, false, state);
                }
            }
            TriggerMode::Tap => {
                if let Some(dur) = release_duration {
                    if dur <= TAP_THRESHOLD {
                        fire(app, registry, &payload, btn, mode, true, state);
                    }
                }
            }
            TriggerMode::DoubleTap => {
                if let Some(dur) = release_duration {
                    if dur > TAP_THRESHOLD {
                        entry.last_release = None;
                    } else if let Some(prev_release) = entry.last_release {
                        if now.duration_since(prev_release) <= DOUBLE_TAP_WINDOW {
                            fire(app, registry, &payload, btn, mode, true, state);
                            entry.last_release = None;
                        } else {
                            entry.last_release = Some(now);
                        }
                    } else {
                        entry.last_release = Some(now);
                    }
                }
            }
            TriggerMode::LongPress => {
                if pressed {
                    entry.long_fired = false;
                } else {
                    entry.long_fired = false;
                }
            }
            TriggerMode::Repeat => {
                if pressed {
                    entry.last_repeat_at = Some(now);
                    fire(app, registry, &payload, btn, mode, true, state);
                } else {
                    entry.last_repeat_at = None;
                }
            }
        }
    }

    if pressed {
        entry.pressed_at = Some(now);
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
    let mappings = lookup_mappings_for_button(state, btn);
    if mappings.is_empty() {
        return;
    }
    let Some(entry) = fsm.get_mut(&btn) else {
        return;
    };
    let Some(start) = entry.pressed_at else {
        return;
    };

    for (payload, mode) in mappings {
        match mode {
            TriggerMode::LongPress => {
                if entry.long_fired {
                    continue;
                }
                if now.duration_since(start) >= LONG_PRESS_THRESHOLD {
                    fire(app, registry, &payload, btn, mode, true, state);
                    entry.long_fired = true;
                }
            }
            TriggerMode::Repeat => {
                // Wait initial delay (350ms) before starting to repeat,
                // then fire every interval (50ms) while held.
                if now.duration_since(start) < REPEAT_INITIAL_DELAY {
                    continue;
                }
                let last = entry.last_repeat_at.unwrap_or(start);
                if now.duration_since(last) >= REPEAT_INTERVAL {
                    fire(app, registry, &payload, btn, mode, true, state);
                    entry.last_repeat_at = Some(now);
                }
            }
            _ => {}
        }
    }
}

fn lookup_mappings_for_button(
    state: &State,
    btn: JoyConButton,
) -> Vec<(ActionPayload, TriggerMode)> {
    state
        .active_mappings()
        .iter()
        .filter(|m| m.button == btn)
        .filter_map(|m| Some((m.payload.clone()?, m.mode)))
        .collect()
}

fn fire<R: Runtime>(
    app: &AppHandle<R>,
    registry: &Arc<ActionRegistry<R>>,
    payload: &ActionPayload,
    button: JoyConButton,
    mode: TriggerMode,
    pressed: bool,
    state: &State,
) {
    dispatch(app, registry, payload, button, mode, pressed);
    // Request rumble if enabled
    if pressed && state.rumble_enabled.load(Ordering::Relaxed) {
        state.rumble_pending.store(true, Ordering::Relaxed);
    }
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
        | JoyConButton::ShakeVertical
        | JoyConButton::IrProximity
        | JoyConButton::NfcTagPresent => false,
    }
}

/// Joy-Con L only exposes the left stick; R only the right; Pro has both.
fn side_has_left_stick(side: JoyConSide) -> bool {
    matches!(side, JoyConSide::Left | JoyConSide::Pro)
}

fn side_has_right_stick(side: JoyConSide) -> bool {
    matches!(side, JoyConSide::Right | JoyConSide::Pro)
}

/// Skip buttons that do not exist on this controller side (avoids garbage analog on the absent stick).
fn button_relevant_for_side(btn: JoyConButton, side: JoyConSide) -> bool {
    match btn {
        JoyConButton::LStick
        | JoyConButton::LStickUp
        | JoyConButton::LStickDown
        | JoyConButton::LStickLeft
        | JoyConButton::LStickRight => side_has_left_stick(side),
        JoyConButton::RStick
        | JoyConButton::RStickUp
        | JoyConButton::RStickDown
        | JoyConButton::RStickLeft
        | JoyConButton::RStickRight => side_has_right_stick(side),
        JoyConButton::A
        | JoyConButton::B
        | JoyConButton::X
        | JoyConButton::Y
        | JoyConButton::Plus
        | JoyConButton::Home
        | JoyConButton::R
        | JoyConButton::Zr
        | JoyConButton::SlRight
        | JoyConButton::SrRight => matches!(side, JoyConSide::Right | JoyConSide::Pro),
        JoyConButton::Minus
        | JoyConButton::Capture
        | JoyConButton::L
        | JoyConButton::Zl
        | JoyConButton::SlLeft
        | JoyConButton::SrLeft
        | JoyConButton::Up
        | JoyConButton::Down
        | JoyConButton::Left
        |         JoyConButton::Right => matches!(side, JoyConSide::Left | JoyConSide::Pro),
        JoyConButton::IrProximity | JoyConButton::NfcTagPresent => matches!(side, JoyConSide::Right),
        _ => true,
    }
}

/// Step 1: median stick readings over 500 ms while the controller rests at connect time.
fn calibrate_sticks<R: Runtime>(
    mode_full: &mut StandardFullMode<SimpleJoyConDriver>,
    app: &AppHandle<R>,
    state: &State,
    kind: ControllerKind,
    serial: &str,
    side: JoyConSide,
) -> JoyConResult<StickCalibration> {
    let sample_left = side_has_left_stick(side);
    let sample_right = side_has_right_stick(side);
    let mut left_h: Vec<i32> = Vec::new();
    let mut left_v: Vec<i32> = Vec::new();
    let mut right_h: Vec<i32> = Vec::new();
    let mut right_v: Vec<i32> = Vec::new();
    let end = Instant::now() + STICK_CALIBRATION;

    while state.running.load(Ordering::Relaxed) && Instant::now() < end {
        match read_report(mode_full) {
            Ok(report) => {
                if report.common.input_report_id != 0x30 {
                    continue;
                }
                if sample_left {
                    let l = &report.common.left_analog_stick_data;
                    left_h.push(l.horizontal as i32);
                    left_v.push(l.vertical as i32);
                }
                if sample_right {
                    let r = &report.common.right_analog_stick_data;
                    right_h.push(r.horizontal as i32);
                    right_v.push(r.vertical as i32);
                }
                mark_connected_if_standard(app, state, &report, kind, serial);
                *state.last_seen.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            }
            Err(e) if is_transient_setup_error(&e) => {}
            Err(e) => return Err(e),
        }
        thread::sleep(POLL);
    }

    let has_left = sample_left && !left_h.is_empty();
    let has_right = sample_right && !right_h.is_empty();
    if !has_left && !has_right {
        warn!("[joycon] stick calibration ({serial}, {side:?}): no samples, using nominal center");
        return Ok(StickCalibration::nominal());
    }

    let mut cal = StickCalibration::nominal();
    if has_left {
        cal.left_h_center = robust_center(&left_h);
        cal.left_v_center = robust_center(&left_v);
    }
    if has_right {
        cal.right_h_center = robust_center(&right_h);
        cal.right_v_center = robust_center(&right_v);
    }
    info!(
        "[joycon] stick calibrated ({serial}, {side:?}): L({}, {}) R({}, {})",
        cal.left_h_center, cal.left_v_center, cal.right_h_center, cal.right_v_center
    );
    Ok(cal)
}

/// Median of samples, dropping outliers far from the median (e.g. accidental nudge during calib).
fn robust_center(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return STICK_CENTER;
    }
    let med = median_i32(samples);
    const OUTLIER: i32 = 350;
    let trimmed: Vec<i32> = samples
        .iter()
        .copied()
        .filter(|v| (*v - med).abs() <= OUTLIER)
        .collect();
    if trimmed.len() >= samples.len() / 2 {
        median_i32(&trimmed)
    } else {
        med
    }
}

fn median_i32(samples: &[i32]) -> i32 {
    if samples.is_empty() {
        return STICK_CENTER;
    }
    let mut sorted: Vec<i32> = samples.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn stick_delta_for(
    report: &StandardInputReport<IMUData>,
    btn: JoyConButton,
    cal: &StickCalibration,
) -> Option<(i32, i32)> {
    match btn {
        JoyConButton::LStick
        | JoyConButton::LStickUp
        | JoyConButton::LStickDown
        | JoyConButton::LStickLeft
        | JoyConButton::LStickRight => {
            let s = &report.common.left_analog_stick_data;
            Some((
                s.horizontal as i32 - cal.left_h_center,
                s.vertical as i32 - cal.left_v_center,
            ))
        }
        JoyConButton::RStick
        | JoyConButton::RStickUp
        | JoyConButton::RStickDown
        | JoyConButton::RStickLeft
        | JoyConButton::RStickRight => {
            let s = &report.common.right_analog_stick_data;
            Some((
                s.horizontal as i32 - cal.right_h_center,
                s.vertical as i32 - cal.right_v_center,
            ))
        }
        _ => None,
    }
}

/// True if the analog stick associated with `btn` is tilted past the threshold.
/// Treats LStick / RStick as "pressed" when their analog data drifts from center.
fn stick_tilted(
    report: &StandardInputReport<IMUData>,
    btn: JoyConButton,
    cal: &StickCalibration,
) -> bool {
    let Some((dx, dy)) = stick_delta_for(report, btn, cal) else {
        return false;
    };
    if !matches!(btn, JoyConButton::LStick | JoyConButton::RStick) {
        return false;
    }
    dx.abs() > STICK_TILT_THRESHOLD || dy.abs() > STICK_TILT_THRESHOLD
}

/// Virtual directional buttons from analog stick (independent of click).
fn stick_direction(
    report: &StandardInputReport<IMUData>,
    btn: JoyConButton,
    cal: &StickCalibration,
    was_pressed: bool,
) -> bool {
    let dir = match btn {
        JoyConButton::LStickUp | JoyConButton::RStickUp => "up",
        JoyConButton::LStickDown | JoyConButton::RStickDown => "down",
        JoyConButton::LStickLeft | JoyConButton::RStickLeft => "left",
        JoyConButton::LStickRight | JoyConButton::RStickRight => "right",
        _ => return false,
    };
    let Some((dx, dy)) = stick_delta_for(report, btn, cal) else {
        return false;
    };
    let threshold = if was_pressed {
        STICK_DIR_RELEASE_THRESHOLD
    } else {
        STICK_DIR_PRESS_THRESHOLD
    };
    match dir {
        "up" => dy > threshold,
        "down" => dy < -threshold,
        "right" => dx > threshold,
        "left" => dx < -threshold,
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
    let imu = state.imu.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let axis = &report.extra.data[0];
    let accel_mag2 =
        (axis.accel_x as i32).pow(2) + (axis.accel_y as i32).pow(2) + (axis.accel_z as i32).pow(2);
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
        trigger(
            JoyConButton::ShakeHorizontal,
            gesture_until,
            last_gesture_at,
        );
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

/// Apply LED pattern to Joy-Con player lights.
/// 0=off, 1=idle(LED0 solid), 2=recording(all flash), 3=low battery(LED0 flash)
fn apply_led_pattern(driver: &mut SimpleJoyConDriver, pattern: u8) {
    use joycon_rs::prelude::lights::*;
    let result = match pattern {
        0 => driver.set_player_lights(&[], &[]),
        1 => driver.set_player_lights(&[LightUp::LED0], &[]),
        2 => driver.set_player_lights(&[], &[Flash::LED0, Flash::LED1, Flash::LED2, Flash::LED3]),
        3 => driver.set_player_lights(&[], &[Flash::LED0]),
        _ => driver.set_player_lights(&[LightUp::LED0], &[]),
    };
    if let Err(e) = result {
        warn!("[joycon] LED set failed: {e:?}");
    }
}

/// Send a short rumble pulse.
fn rumble_pulse(driver: &mut SimpleJoyConDriver) {
    let rumble = Rumble::new(200.0, 0.6);
    let _ = driver.rumble((Some(rumble), Some(rumble)));
}

/// Stop rumble (send zero amplitude).
fn rumble_stop(driver: &mut SimpleJoyConDriver) {
    let silent = Rumble::new(160.0, 0.0);
    let _ = driver.rumble((Some(silent), Some(silent)));
}

#[cfg(target_os = "macos")]
pub fn spawn_frontmost_watcher<R: Runtime>(app: AppHandle<R>, state: State) {
    thread::spawn(move || {
        let mut last: Option<String> = None;
        while state.running.load(Ordering::Relaxed) {
            let bundle = read_frontmost_bundle();
            if bundle != last {
                *state.frontmost_bundle.lock().unwrap_or_else(|e| e.into_inner()) = bundle.clone();
                let _ = app.emit("joycon://frontmost_changed", bundle.clone());
                last = bundle;
            }
            thread::sleep(Duration::from_millis(800));
        }
    });
}

#[cfg(target_os = "macos")]
fn read_frontmost_bundle() -> Option<String> {
    let asn = std::process::Command::new("lsappinfo")
        .arg("front")
        .output()
        .ok()?;
    let asn_str = String::from_utf8_lossy(&asn.stdout).trim().to_string();
    if asn_str.is_empty() {
        return None;
    }
    let out = std::process::Command::new("lsappinfo")
        .args(["info", "-only", "bundleid", &asn_str])
        .output()
        .ok()?;
    // Output format: "CFBundleIdentifier"="com.example.app"
    let s = String::from_utf8_lossy(&out.stdout);
    s.split('"').nth(3).map(|b| b.to_string())
}

#[cfg(target_os = "macos")]
pub fn read_frontmost_app() -> Option<(String, String)> {
    let asn = std::process::Command::new("lsappinfo")
        .arg("front")
        .output()
        .ok()?;
    let asn_str = String::from_utf8_lossy(&asn.stdout).trim().to_string();
    if asn_str.is_empty() {
        return None;
    }
    let out = std::process::Command::new("lsappinfo")
        .args(["info", "-only", "bundleid", "-only", "name", &asn_str])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut bundle = String::new();
    let mut name = String::new();
    for line in text.lines() {
        if line.contains("CFBundleIdentifier") {
            if let Some(v) = line.split('"').nth(3) {
                bundle = v.to_string();
            }
        } else if line.contains("LSDisplayName") {
            if let Some(v) = line.split('"').nth(3) {
                name = v.to_string();
            }
        }
    }
    if bundle.is_empty() {
        None
    } else {
        if name.is_empty() {
            name = bundle.clone();
        }
        Some((name, bundle))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_frontmost_app() -> Option<(String, String)> {
    None
}
