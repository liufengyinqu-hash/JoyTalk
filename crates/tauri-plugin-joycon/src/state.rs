//! Shared atomic / mutex state across listener threads + commands.

use crate::types::{
    AppProfile, ButtonMapping, ConnectedController, ControllerKind, ImuConfig, IrLiveSample,
    JoyConStatus, McuConfig, McuMode, McuRuntime, McuStatus, NfcLiveSample,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU8};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct State {
    pub mappings: Arc<Mutex<Vec<ButtonMapping>>>,
    pub enabled: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub connected: Arc<AtomicBool>,
    pub battery_pct: Arc<AtomicU8>,
    pub charging: Arc<AtomicBool>,
    pub device_count: Arc<AtomicU8>,
    pub last_seen: Arc<Mutex<Instant>>,
    pub capture_mode: Arc<AtomicBool>,
    pub seen_serials: Arc<Mutex<HashSet<String>>>,
    pub imu: Arc<Mutex<ImuConfig>>,
    pub mcu: Arc<Mutex<McuConfig>>,
    pub mcu_runtime: Arc<Mutex<McuRuntime>>,
    pub ir_sample: Arc<Mutex<IrLiveSample>>,
    pub nfc_sample: Arc<Mutex<NfcLiveSample>>,
    pub nfc_rescan: Arc<AtomicBool>,
    pub profiles: Arc<Mutex<Vec<AppProfile>>>,
    pub per_app_enabled: Arc<AtomicBool>,
    pub frontmost_bundle: Arc<Mutex<Option<String>>>,
    pub connected_controllers: Arc<Mutex<Vec<ConnectedController>>>,
    /// LED pattern: 0=off, 1=idle(LED0), 2=recording(all flash), 3=low battery(LED0 flash)
    pub led_pattern: Arc<AtomicU8>,
    /// Rumble feedback enabled
    pub rumble_enabled: Arc<AtomicBool>,
    /// Set by fire() to request a rumble pulse from the drive loop
    pub rumble_pending: Arc<AtomicBool>,
}

impl State {
    pub fn new(
        enabled: bool,
        mappings: Vec<ButtonMapping>,
        seen_serials: HashSet<String>,
        imu: ImuConfig,
        mcu: McuConfig,
        profiles: Vec<AppProfile>,
        per_app_enabled: bool,
    ) -> Self {
        Self {
            mappings: Arc::new(Mutex::new(mappings)),
            enabled: Arc::new(AtomicBool::new(enabled)),
            running: Arc::new(AtomicBool::new(true)),
            connected: Arc::new(AtomicBool::new(false)),
            battery_pct: Arc::new(AtomicU8::new(0)),
            charging: Arc::new(AtomicBool::new(false)),
            device_count: Arc::new(AtomicU8::new(0)),
            last_seen: Arc::new(Mutex::new(Instant::now())),
            capture_mode: Arc::new(AtomicBool::new(false)),
            seen_serials: Arc::new(Mutex::new(seen_serials)),
            imu: Arc::new(Mutex::new(imu)),
            mcu: Arc::new(Mutex::new(mcu)),
            mcu_runtime: Arc::new(Mutex::new(McuRuntime::default())),
            ir_sample: Arc::new(Mutex::new(IrLiveSample::default())),
            nfc_sample: Arc::new(Mutex::new(NfcLiveSample::default())),
            nfc_rescan: Arc::new(AtomicBool::new(false)),
            profiles: Arc::new(Mutex::new(profiles)),
            per_app_enabled: Arc::new(AtomicBool::new(per_app_enabled)),
            frontmost_bundle: Arc::new(Mutex::new(None)),
            connected_controllers: Arc::new(Mutex::new(Vec::new())),
            led_pattern: Arc::new(AtomicU8::new(1)), // idle = LED0
            rumble_enabled: Arc::new(AtomicBool::new(true)),
            rumble_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn register_controller(&self, kind: ControllerKind, serial: String) {
        let mut list = self.connected_controllers.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|c| c.serial != serial);
        list.push(ConnectedController { kind, serial });
    }

    pub fn unregister_controller(&self, serial: &str) {
        let mut list = self.connected_controllers.lock().unwrap_or_else(|e| e.into_inner());
        list.retain(|c| c.serial != serial);
    }

    pub fn clear_controllers(&self) {
        self.connected_controllers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        *self.ir_sample.lock().unwrap_or_else(|e| e.into_inner()) = IrLiveSample::default();
        *self.nfc_sample.lock().unwrap_or_else(|e| e.into_inner()) = NfcLiveSample::default();
        self.set_mcu_runtime(McuRuntime::default());
    }

    pub fn mcu_runtime_snapshot(&self) -> McuRuntime {
        *self.mcu_runtime.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn mcu_status_snapshot(&self) -> McuStatus {
        let config = self.mcu.lock().unwrap_or_else(|e| e.into_inner()).clone();
        McuStatus::from_parts(config, self.mcu_runtime_snapshot())
    }

    pub fn set_mcu_runtime(&self, runtime: McuRuntime) {
        *self.mcu_runtime.lock().unwrap_or_else(|e| e.into_inner()) = runtime;
    }

    pub fn set_ir_sample(&self, sample: IrLiveSample) {
        *self.ir_sample.lock().unwrap_or_else(|e| e.into_inner()) = sample;
    }

    pub fn set_nfc_sample(&self, sample: NfcLiveSample) {
        *self.nfc_sample.lock().unwrap_or_else(|e| e.into_inner()) = sample;
    }

    pub fn snapshot(&self) -> JoyConStatus {
        use std::sync::atomic::Ordering::Relaxed;
        let connected_controllers = self
            .connected_controllers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        JoyConStatus {
            connected: self.connected.load(Relaxed),
            battery: self.battery_pct.load(Relaxed),
            charging: self.charging.load(Relaxed),
            device_count: self.device_count.load(Relaxed),
            connected_controllers,
        }
    }

    /// Returns the active mappings honoring per-app profile if matched.
    pub fn active_mappings(&self) -> Vec<ButtonMapping> {
        use std::sync::atomic::Ordering::Relaxed;
        if self.per_app_enabled.load(Relaxed) {
            let front = self.frontmost_bundle.lock().unwrap_or_else(|e| e.into_inner());
            let profiles = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(b) = front.as_deref() {
                if let Some(p) = profiles.iter().find(|p| p.bundle_id == b) {
                    return p.mappings.clone();
                }
            }
        }
        self.mappings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
