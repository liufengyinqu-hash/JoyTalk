//! Action registry: consumer registers builtin handlers; payload dispatches to
//! builtin / keyboard / text via injector.

use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Runtime};

use crate::injector;
use crate::types::{ActionPayload, JoyConButton, TriggerMode};

pub struct ActionContext<R: Runtime> {
    pub app: AppHandle<R>,
    pub button: JoyConButton,
    pub mode: TriggerMode,
    pub pressed: bool,
}

pub type ActionFn<R> = Arc<dyn Fn(ActionContext<R>) + Send + Sync + 'static>;

pub struct ActionRegistry<R: Runtime> {
    actions: HashMap<String, ActionFn<R>>,
}

impl<R: Runtime> Default for ActionRegistry<R> {
    fn default() -> Self {
        Self {
            actions: HashMap::new(),
        }
    }
}

impl<R: Runtime> Clone for ActionRegistry<R> {
    fn clone(&self) -> Self {
        Self {
            actions: self.actions.clone(),
        }
    }
}

impl<R: Runtime> ActionRegistry<R> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<F>(&mut self, id: impl Into<String>, handler: F)
    where
        F: Fn(ActionContext<R>) + Send + Sync + 'static,
    {
        self.actions.insert(id.into(), Arc::new(handler));
    }

    pub fn get(&self, id: &str) -> Option<ActionFn<R>> {
        self.actions.get(id).cloned()
    }

    pub fn ids(&self) -> Vec<String> {
        self.actions.keys().cloned().collect()
    }
}

/// Dispatch a payload. Hold mode emits press/release pairs; tap/double/long
/// only fire on the "fire" edge (always pressed=true).
pub fn dispatch<R: Runtime>(
    app: &AppHandle<R>,
    registry: &ActionRegistry<R>,
    payload: &ActionPayload,
    button: JoyConButton,
    mode: TriggerMode,
    pressed: bool,
) {
    match payload {
        ActionPayload::Builtin { id } => {
            if let Some(handler) = registry.get(id) {
                handler(ActionContext {
                    app: app.clone(),
                    button,
                    mode,
                    pressed,
                });
            }
        }
        ActionPayload::Keyboard { chords } => {
            // Macros only fire once per trigger; ignore release edge in Hold mode
            if pressed {
                injector::inject_keyboard(chords);
            }
        }
        ActionPayload::Text { text } => {
            if pressed {
                injector::inject_text(text);
            }
        }
        ActionPayload::OpenApp { bundle_id } => {
            if pressed {
                injector::open_app(bundle_id);
            }
        }
        ActionPayload::Shell { command } => {
            if pressed {
                injector::run_shell(command);
            }
        }
        ActionPayload::AppleScript { script } => {
            if pressed {
                injector::run_applescript(script);
            }
        }
    }
}

pub fn payload_action_id(payload: &ActionPayload) -> Option<&str> {
    if let ActionPayload::Builtin { id } = payload {
        Some(id.as_str())
    } else {
        None
    }
}
