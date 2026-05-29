//! Shared atomic / mutex state across listener threads + commands.

use crate::types::{AppProfile, ButtonMapping, ConnectedController, ControllerKind, ImuConfig, JoyConStatus};
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
    pub profiles: Arc<Mutex<Vec<AppProfile>>>,
    pub per_app_enabled: Arc<AtomicBool>,
    pub frontmost_bundle: Arc<Mutex<Option<String>>>,
    pub connected_controllers: Arc<Mutex<Vec<ConnectedController>>>,
}

impl State {
    pub fn new(
        enabled: bool,
        mappings: Vec<ButtonMapping>,
        seen_serials: HashSet<String>,
        imu: ImuConfig,
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
            profiles: Arc::new(Mutex::new(profiles)),
            per_app_enabled: Arc::new(AtomicBool::new(per_app_enabled)),
            frontmost_bundle: Arc::new(Mutex::new(None)),
            connected_controllers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn register_controller(&self, kind: ControllerKind, serial: String) {
        if let Ok(mut list) = self.connected_controllers.lock() {
            list.retain(|c| c.serial != serial);
            list.push(ConnectedController { kind, serial });
        }
    }

    pub fn unregister_controller(&self, serial: &str) {
        if let Ok(mut list) = self.connected_controllers.lock() {
            list.retain(|c| c.serial != serial);
        }
    }

    pub fn clear_controllers(&self) {
        if let Ok(mut list) = self.connected_controllers.lock() {
            list.clear();
        }
    }

    pub fn snapshot(&self) -> JoyConStatus {
        use std::sync::atomic::Ordering::Relaxed;
        let connected_controllers = self
            .connected_controllers
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
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
            if let (Ok(front), Ok(profiles)) = (
                self.frontmost_bundle.lock(),
                self.profiles.lock(),
            ) {
                if let Some(b) = front.as_deref() {
                    if let Some(p) = profiles.iter().find(|p| p.bundle_id == b) {
                        return p.mappings.clone();
                    }
                }
            }
        }
        self.mappings
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}
