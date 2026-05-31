//! Tauri commands exposed to frontend.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime, State as TState};

use crate::presets::{builtin_summaries, find_builtin, validate};
use crate::settings::{save, PluginConfig};
use crate::state::State;
use crate::types::{
    ActionPayload, AppProfile, ButtonMapping, ImuConfig, IrLiveSample, JoyConButton, McuConfig,
    McuStatus, NfcLiveSample, JoyConStatus, Preset, PresetMapping, PresetSummary, TriggerMode,
};

#[tauri::command]
#[specta::specta]
pub fn joycon_get_status(state: TState<'_, State>) -> JoyConStatus {
    state.snapshot()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_mappings(state: TState<'_, State>) -> Vec<ButtonMapping> {
    state.mappings.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_set_mapping<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    button: JoyConButton,
    payload: Option<ActionPayload>,
    mode: TriggerMode,
) -> Result<(), String> {
    {
        let mut guard = state.mappings.lock().map_err(|_| "lock poisoned")?;
        if payload.is_none() {
            guard.retain(|m| !(m.button == button && m.mode == mode));
        } else if let Some(m) = guard
            .iter_mut()
            .find(|m| m.button == button && m.mode == mode)
        {
            m.payload = payload.clone();
        } else {
            guard.push(ButtonMapping {
                button,
                payload: payload.clone(),
                mode,
            });
        }
    }
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_reset_mappings<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
) -> Result<Vec<ButtonMapping>, String> {
    let defaults = crate::types::default_mappings();
    {
        let mut guard = state.mappings.lock().map_err(|_| "lock poisoned")?;
        *guard = defaults.clone();
    }
    persist(&app, &state)?;
    Ok(defaults)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_set_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    enabled: bool,
) -> Result<(), String> {
    state.enabled.store(enabled, Ordering::Relaxed);
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_enabled(state: TState<'_, State>) -> bool {
    state.enabled.load(Ordering::Relaxed)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_list_actions(action_ids: TState<'_, Arc<Vec<String>>>) -> Vec<String> {
    (**action_ids).clone()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_start_capture(state: TState<'_, State>) {
    state.capture_mode.store(true, Ordering::Relaxed);
}

#[tauri::command]
#[specta::specta]
pub fn joycon_stop_capture(state: TState<'_, State>) {
    state.capture_mode.store(false, Ordering::Relaxed);
}

#[tauri::command]
#[specta::specta]
pub fn joycon_list_presets() -> Vec<PresetSummary> {
    builtin_summaries()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_preset_mappings(preset_id: String) -> Result<Vec<PresetMapping>, String> {
    find_builtin(&preset_id)
        .map(|p| p.mappings)
        .ok_or_else(|| format!("unknown preset: {preset_id}"))
}

#[tauri::command]
#[specta::specta]
pub fn joycon_load_preset<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    preset_id: String,
) -> Result<(), String> {
    let preset = find_builtin(&preset_id).ok_or_else(|| format!("unknown preset: {preset_id}"))?;
    apply_preset(&app, &state, preset)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_load_preset_from_url<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    url: String,
) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("URL must use https".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 {
        return Err("preset too large (>64KB)".into());
    }
    let preset: Preset = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    validate(&preset)?;
    apply_preset(&app, &state, preset)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_export_mappings(state: TState<'_, State>) -> Result<String, String> {
    let mappings = state.mappings.lock().map(|g| g.clone()).unwrap_or_default();
    let preset = Preset {
        id: "custom-export".into(),
        name: "Custom Export".into(),
        description: "Exported from JoyTalk".into(),
        kind: "any".into(),
        mappings: mappings
            .into_iter()
            .filter_map(|m| match m.payload {
                Some(ActionPayload::Builtin { id }) => Some(PresetMapping {
                    button: m.button,
                    action: id,
                    mode: m.mode,
                }),
                _ => None,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&preset).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn joycon_import_mappings<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    json: String,
) -> Result<(), String> {
    let preset: Preset = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    validate(&preset)?;
    apply_preset(&app, &state, preset)
}

fn apply_preset<R: Runtime>(
    app: &AppHandle<R>,
    state: &State,
    preset: Preset,
) -> Result<(), String> {
    let mut new_mappings: Vec<ButtonMapping> = preset
        .mappings
        .into_iter()
        .map(|m| ButtonMapping {
            button: m.button,
            payload: Some(ActionPayload::Builtin { id: m.action }),
            mode: m.mode,
        })
        .collect();
    {
        let mut guard = state.mappings.lock().map_err(|_| "lock poisoned")?;
        std::mem::swap(&mut *guard, &mut new_mappings);
    }
    persist(app, state)
}

fn persist<R: Runtime>(app: &AppHandle<R>, state: &State) -> Result<(), String> {
    let cfg = PluginConfig {
        config_version: crate::settings::current_config_version(),
        enabled: state.enabled.load(Ordering::Relaxed),
        mappings: state.mappings.lock().map(|g| g.clone()).unwrap_or_default(),
        seen_serials: state
            .seen_serials
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default(),
        imu: state.imu.lock().map(|g| g.clone()).unwrap_or_default(),
        mcu: state.mcu.lock().map(|g| g.clone()).unwrap_or_default(),
        profiles: state.profiles.lock().map(|g| g.clone()).unwrap_or_default(),
        per_app_enabled: state.per_app_enabled.load(Ordering::Relaxed),
    };
    save(app, &cfg)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_imu(state: TState<'_, State>) -> ImuConfig {
    state.imu.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_set_imu<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    imu: ImuConfig,
) -> Result<(), String> {
    if let Ok(mut g) = state.imu.lock() {
        *g = imu;
    }
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_ir(state: TState<'_, State>) -> McuStatus {
    state.mcu_status_snapshot()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_set_ir<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    ir: McuConfig,
) -> Result<(), String> {
    if let Ok(mut g) = state.mcu.lock() {
        *g = ir;
    }
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_ir_sample(state: TState<'_, State>) -> IrLiveSample {
    state.ir_sample.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_nfc_sample(state: TState<'_, State>) -> NfcLiveSample {
    state.nfc_sample.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_restart_nfc_scan(state: TState<'_, State>) {
    state.nfc_rescan.store(true, Ordering::Relaxed);
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_profiles(state: TState<'_, State>) -> Vec<AppProfile> {
    state.profiles.lock().map(|g| g.clone()).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn joycon_save_profile<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    bundle_id: String,
    mappings: Vec<ButtonMapping>,
) -> Result<(), String> {
    if bundle_id.trim().is_empty() {
        return Err("bundle_id empty".into());
    }
    if let Ok(mut g) = state.profiles.lock() {
        if let Some(p) = g.iter_mut().find(|p| p.bundle_id == bundle_id) {
            p.mappings = mappings;
        } else {
            g.push(AppProfile {
                bundle_id,
                mappings,
            });
        }
    }
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_delete_profile<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    bundle_id: String,
) -> Result<(), String> {
    if let Ok(mut g) = state.profiles.lock() {
        g.retain(|p| p.bundle_id != bundle_id);
    }
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_set_per_app_enabled<R: Runtime>(
    app: AppHandle<R>,
    state: TState<'_, State>,
    enabled: bool,
) -> Result<(), String> {
    state.per_app_enabled.store(enabled, Ordering::Relaxed);
    persist(&app, &state)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_per_app_enabled(state: TState<'_, State>) -> bool {
    state.per_app_enabled.load(Ordering::Relaxed)
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_frontmost(state: TState<'_, State>) -> Option<String> {
    state.frontmost_bundle.lock().ok().and_then(|g| g.clone())
}

#[derive(serde::Serialize, specta::Type)]
pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: String,
}

#[tauri::command]
#[specta::specta]
pub fn joycon_get_frontmost_app() -> Option<FrontmostApp> {
    crate::listener::read_frontmost_app().map(|(name, bundle_id)| FrontmostApp { name, bundle_id })
}

pub fn provide_state<R: Runtime>(
    app: &AppHandle<R>,
    state: State,
    registry: Arc<crate::actions::ActionRegistry<R>>,
) {
    app.manage(state);
    app.manage(registry);
}

#[derive(serde::Serialize, specta::Type)]
pub struct AppEntry {
    pub name: String,
    pub bundle_id: String,
    pub path: String,
}

#[tauri::command]
#[specta::specta]
pub fn joycon_list_apps() -> Vec<AppEntry> {
    #[cfg(target_os = "macos")]
    {
        list_macos_apps()
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![]
    }
}

#[cfg(target_os = "macos")]
fn list_macos_apps() -> Vec<AppEntry> {
    use std::fs;
    use std::path::Path;
    let mut out = Vec::new();
    for dir in ["/Applications", "/System/Applications"] {
        let p = Path::new(dir);
        let Ok(entries) = fs::read_dir(p) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("app") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let bundle_id = read_bundle_id(&path).unwrap_or_default();
            out.push(AppEntry {
                name,
                bundle_id,
                path: path.to_string_lossy().to_string(),
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[cfg(target_os = "macos")]
fn read_bundle_id(app_path: &std::path::Path) -> Option<String> {
    let plist = app_path.join("Contents/Info.plist");
    let bytes = std::fs::read(&plist).ok()?;
    // crude scan: look for CFBundleIdentifier string value
    let s = String::from_utf8_lossy(&bytes);
    let needle = "<key>CFBundleIdentifier</key>";
    let pos = s.find(needle)?;
    let tail = &s[pos + needle.len()..];
    let start = tail.find("<string>")? + "<string>".len();
    let end = tail[start..].find("</string>")?;
    Some(tail[start..start + end].trim().to_string())
}
