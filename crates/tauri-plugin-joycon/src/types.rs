//! Public types: buttons, mapping, status, events.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Type)]
#[serde(rename_all = "snake_case")]
pub enum JoyConButton {
    A,
    B,
    X,
    Y,
    Plus,
    Minus,
    Home,
    Capture,
    L,
    R,
    Zl,
    Zr,
    LStick,
    RStick,
    SlLeft,
    SrLeft,
    SlRight,
    SrRight,
    Up,
    Down,
    Left,
    Right,
    // Analog stick virtual directional keys (left)
    LStickUp,
    LStickDown,
    LStickLeft,
    LStickRight,
    // Analog stick virtual directional keys (right)
    RStickUp,
    RStickDown,
    RStickLeft,
    RStickRight,
    // IMU gesture buttons (fired on detection, persistent for 200ms)
    Shake,
    FlipUp,
    FlipDown,
    TiltLeft,
    TiltRight,
    ShakeHorizontal,
    ShakeVertical,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    Hold,
    Tap,
    DoubleTap,
    LongPress,
    /// Press fires once immediately, then repeats every interval while held.
    /// Like system key-repeat (e.g. Delete key auto-deleting characters).
    Repeat,
}

impl Default for TriggerMode {
    fn default() -> Self {
        TriggerMode::Hold
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Cmd,
    Ctrl,
    Alt,
    Shift,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct KeyChord {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionPayload {
    Builtin { id: String },
    Keyboard { chords: Vec<KeyChord> },
    Text { text: String },
    OpenApp { bundle_id: String },
    Shell { command: String },
    AppleScript { script: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ButtonMapping {
    pub button: JoyConButton,
    #[serde(default)]
    pub payload: Option<ActionPayload>,
    #[serde(default)]
    pub mode: TriggerMode,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ConnectedController {
    pub kind: ControllerKind,
    pub serial: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, tauri_specta::Event)]
pub struct JoyConStatus {
    pub connected: bool,
    pub battery: u8,
    pub charging: bool,
    pub device_count: u8,
    /// Controllers currently registered by the listener (survives UI remount).
    #[serde(default)]
    pub connected_controllers: Vec<ConnectedController>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Type)]
#[serde(rename_all = "snake_case")]
pub enum JoyConSide {
    Left,
    Right,
    Pro,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ControllerKind {
    JoyConLeft,
    JoyConRight,
    ProController,
    Unknown,
}

impl From<JoyConSide> for ControllerKind {
    fn from(s: JoyConSide) -> Self {
        match s {
            JoyConSide::Left => ControllerKind::JoyConLeft,
            JoyConSide::Right => ControllerKind::JoyConRight,
            JoyConSide::Pro => ControllerKind::ProController,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Type, tauri_specta::Event)]
pub struct JoyConButtonEvent {
    pub button: JoyConButton,
    pub pressed: bool,
    pub device_index: u8,
    pub side: JoyConSide,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, tauri_specta::Event)]
pub struct ControllerDetected {
    pub kind: ControllerKind,
    pub serial: String,
    pub device_index: u8,
    pub is_first_pair: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type, tauri_specta::Event)]
pub struct JoyConActionFired {
    pub action: String,
    pub button: JoyConButton,
    pub mode: TriggerMode,
    pub pressed: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct ImuConfig {
    #[serde(default = "default_shake_threshold")]
    pub shake_threshold: i32,
    #[serde(default = "default_flip_threshold")]
    pub flip_threshold: i32,
    #[serde(default = "default_gesture_cooldown_ms")]
    pub gesture_cooldown_ms: u32,
    #[serde(default = "default_gesture_hold_ms")]
    pub gesture_hold_ms: u32,
}

impl Default for ImuConfig {
    fn default() -> Self {
        Self {
            shake_threshold: default_shake_threshold(),
            flip_threshold: default_flip_threshold(),
            gesture_cooldown_ms: default_gesture_cooldown_ms(),
            gesture_hold_ms: default_gesture_hold_ms(),
        }
    }
}

fn default_shake_threshold() -> i32 { 28000 }
fn default_flip_threshold() -> i32 { 18000 }
fn default_gesture_cooldown_ms() -> u32 { 400 }
fn default_gesture_hold_ms() -> u32 { 180 }

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AppProfile {
    pub bundle_id: String,
    pub mappings: Vec<ButtonMapping>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PresetMapping {
    pub button: JoyConButton,
    pub action: String,
    #[serde(default)]
    pub mode: TriggerMode,
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default = "default_preset_kind")]
    pub kind: String,
    pub mappings: Vec<PresetMapping>,
}

fn default_preset_kind() -> String {
    "any".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub fn default_mappings() -> Vec<ButtonMapping> {
    vec![
        ButtonMapping {
            button: JoyConButton::Zl,
            payload: Some(ActionPayload::Builtin {
                id: "transcribe".into(),
            }),
            mode: TriggerMode::Hold,
        },
        ButtonMapping {
            button: JoyConButton::Zr,
            payload: Some(ActionPayload::Builtin {
                id: "transcribe".into(),
            }),
            mode: TriggerMode::Hold,
        },
    ]
}
