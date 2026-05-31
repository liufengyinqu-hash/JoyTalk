//! Persistence: tauri-plugin-store backed mapping table + macros + seen serials.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use crate::types::{default_mappings, AppProfile, ButtonMapping, ImuConfig, McuConfig, McuMode};

const STORE_PATH: &str = ".joycon-plugin.dat";
const KEY: &str = "joycon";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginConfig {
    #[serde(default = "default_missing_config_version")]
    pub config_version: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_mappings")]
    pub mappings: Vec<ButtonMapping>,
    #[serde(default)]
    pub seen_serials: HashSet<String>,
    #[serde(default)]
    pub imu: ImuConfig,
    #[serde(default, alias = "ir")]
    pub mcu: McuConfig,
    #[serde(default)]
    pub profiles: Vec<AppProfile>,
    #[serde(default = "default_per_app_enabled")]
    pub per_app_enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn default_config_version() -> u32 {
    2
}

fn default_missing_config_version() -> u32 {
    0
}

pub fn current_config_version() -> u32 {
    2
}

fn default_per_app_enabled() -> bool {
    false
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            config_version: default_config_version(),
            enabled: true,
            mappings: default_mappings(),
            seen_serials: HashSet::new(),
            imu: ImuConfig::default(),
            mcu: McuConfig::default(),
            profiles: Vec::new(),
            per_app_enabled: false,
        }
    }
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> PluginConfig {
    let Ok(store) = app.store(STORE_PATH) else {
        return PluginConfig::default();
    };
    if let Some(v) = store.get(KEY) {
        if let Ok(mut cfg) = serde_json::from_value::<PluginConfig>(v) {
            if migrate_config(&mut cfg) {
                store.set(KEY, serde_json::to_value(&cfg).unwrap_or_default());
            }
            return cfg;
        }
    }
    let cfg = PluginConfig::default();
    store.set(KEY, serde_json::to_value(&cfg).unwrap_or_default());
    cfg
}

fn migrate_config(cfg: &mut PluginConfig) -> bool {
    let mut changed = false;
    if cfg.config_version < 2 {
        if cfg.mcu.mode == McuMode::Off {
            cfg.mcu.mode = McuMode::Nfc;
        }
        cfg.config_version = 2;
        changed = true;
    }
    changed
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &PluginConfig) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    store.set(KEY, serde_json::to_value(cfg).map_err(|e| e.to_string())?);
    Ok(())
}
