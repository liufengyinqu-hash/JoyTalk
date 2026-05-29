//! tauri-plugin-joycon: Joy-Con input plugin for Tauri 2 apps.

mod actions;
mod commands;
mod injector;
mod listener;
mod presets;
mod settings;
mod state;
mod types;

pub use actions::{ActionContext, ActionRegistry};
pub use types::{
    default_mappings, ActionPayload, AppProfile, ButtonMapping, ControllerDetected,
    ControllerKind, ImuConfig, JoyConActionFired, JoyConButton, JoyConButtonEvent, JoyConSide,
    JoyConStatus, KeyChord, Modifier, Preset, PresetMapping, PresetSummary, TriggerMode,
};

use std::sync::Arc;
use tauri::plugin::TauriPlugin;
use tauri::{Manager, Runtime};

use crate::settings::load as load_settings;
use crate::state::State;

pub struct Builder<R: Runtime> {
    registry: ActionRegistry<R>,
}

impl<R: Runtime> Builder<R> {
    pub fn new() -> Self {
        Self {
            registry: ActionRegistry::new(),
        }
    }

    pub fn action<F>(mut self, id: impl Into<String>, handler: F) -> Self
    where
        F: Fn(ActionContext<R>) + Send + Sync + 'static,
    {
        self.registry.insert(id, handler);
        self
    }

    pub fn build(self) -> TauriPlugin<R> {
        let registry = Arc::new(self.registry);

        tauri::plugin::Builder::<R>::new("joycon")
            .invoke_handler(tauri::generate_handler![
                commands::joycon_get_status,
                commands::joycon_get_mappings,
                commands::joycon_set_mapping,
                commands::joycon_reset_mappings,
                commands::joycon_set_enabled,
                commands::joycon_get_enabled,
                commands::joycon_list_actions,
                commands::joycon_start_capture,
                commands::joycon_stop_capture,
                commands::joycon_list_presets,
                commands::joycon_get_preset_mappings,
                commands::joycon_load_preset,
                commands::joycon_load_preset_from_url,
                commands::joycon_export_mappings,
                commands::joycon_import_mappings,
                commands::joycon_list_apps,
                commands::joycon_get_imu,
                commands::joycon_set_imu,
                commands::joycon_get_profiles,
                commands::joycon_save_profile,
                commands::joycon_delete_profile,
                commands::joycon_set_per_app_enabled,
                commands::joycon_get_per_app_enabled,
                commands::joycon_get_frontmost,
                commands::joycon_get_frontmost_app,
            ])
            .setup(move |app, _api| {
                let app_handle = app.app_handle();
                let cfg = load_settings(&app_handle);
                let state = State::new(
                    cfg.enabled,
                    cfg.mappings,
                    cfg.seen_serials,
                    cfg.imu,
                    cfg.profiles,
                    cfg.per_app_enabled,
                );
                let mut ids = registry.ids();
                ids.sort();
                app_handle.manage(state.clone());
                app_handle.manage(registry.clone());
                app_handle.manage(Arc::new(ids));
                listener::spawn_scanner(app_handle.clone(), state.clone(), registry.clone());
                #[cfg(target_os = "macos")]
                listener::spawn_frontmost_watcher(app_handle.clone(), state);
                Ok(())
            })
            .build()
    }
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::<R>::new().build()
}
