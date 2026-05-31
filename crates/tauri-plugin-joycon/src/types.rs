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
    /// IR camera proximity (right Joy-Con, `mcu.mode = ir`).
    IrProximity,
    /// NFC tag present (right Joy-Con, `mcu.mode = nfc`).
    NfcTagPresent,
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

fn default_shake_threshold() -> i32 {
    28000
}
fn default_flip_threshold() -> i32 {
    18000
}
fn default_gesture_cooldown_ms() -> u32 {
    400
}
fn default_gesture_hold_ms() -> u32 {
    180
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum McuMode {
    Off,
    Ir,
    Nfc,
}

impl Default for McuMode {
    fn default() -> Self {
        McuMode::Off
    }
}

#[derive(Serialize, Debug, Clone, Type)]
pub struct McuConfig {
    /// Right Joy-Con MCU mode: IR proximity, NFC tag detect, or off.
    #[serde(default)]
    pub mode: McuMode,
    /// IR only: `white_pixel_count` above this triggers `IrProximity`.
    #[serde(default = "default_ir_white_pixel_threshold")]
    pub white_pixel_threshold: u16,
}

impl Default for McuConfig {
    fn default() -> Self {
        Self {
            mode: McuMode::Off,
            white_pixel_threshold: default_ir_white_pixel_threshold(),
        }
    }
}

impl<'de> Deserialize<'de> for McuConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(default)]
            mode: Option<McuMode>,
            #[serde(default)]
            enabled: Option<bool>,
            #[serde(default = "default_ir_white_pixel_threshold")]
            white_pixel_threshold: u16,
        }
        let h = Helper::deserialize(deserializer)?;
        let mode = h.mode.unwrap_or_else(|| {
            if h.enabled == Some(false) {
                McuMode::Off
            } else {
                McuMode::Off
            }
        });
        Ok(McuConfig {
            mode,
            white_pixel_threshold: h.white_pixel_threshold,
        })
    }
}

/// Latest IR PulseRate sample from the right Joy-Con (for UI / debug).
#[derive(Serialize, Deserialize, Debug, Clone, Default, Type)]
pub struct IrLiveSample {
    /// IR MCU stream is active on a connected right Joy-Con.
    pub session_active: bool,
    pub average_intensity: u8,
    pub white_pixel_count: u16,
    pub ambient_noise_count: u16,
    pub proximity_active: bool,
}

/// Latest NFC sample from the right Joy-Con (for UI / debug).
#[derive(Serialize, Deserialize, Debug, Clone, Default, Type)]
pub struct NfcLiveSample {
    pub session_active: bool,
    pub tag_present: bool,
    /// MCU reports tag info in the field but UID read is not finalized.
    pub tag_detected: bool,
    pub uid: String,
    pub uid_len: u8,
    pub tag_type: u8,
    pub nfc_state: u8,
}

/// Right Joy-Con MCU runtime (may differ from saved config during hot-switch).
#[derive(Serialize, Debug, Clone, Copy, Default, Type)]
pub struct McuRuntime {
    pub active_mode: McuMode,
    pub switching: bool,
}

/// Saved MCU config + live runtime for the settings UI.
#[derive(Serialize, Debug, Clone, Type)]
pub struct McuStatus {
    pub config: McuConfig,
    pub active_mode: McuMode,
    pub switching: bool,
}

impl McuStatus {
    pub fn from_parts(config: McuConfig, runtime: McuRuntime) -> Self {
        Self {
            config,
            active_mode: runtime.active_mode,
            switching: runtime.switching,
        }
    }
}

fn default_ir_white_pixel_threshold() -> u16 {
    50
}

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

#[cfg(test)]
mod mcu_config_tests {
    use super::*;

    #[test]
    fn default_mode_is_off() {
        assert_eq!(McuConfig::default().mode, McuMode::Off);
    }

    #[test]
    fn deserializes_nfc_mode() {
        let cfg: McuConfig =
            serde_json::from_str(r#"{"mode":"nfc","white_pixel_threshold":50}"#).unwrap();
        assert_eq!(cfg.mode, McuMode::Nfc);
        assert_eq!(cfg.white_pixel_threshold, 50);
    }

    #[test]
    fn deserializes_off_via_legacy_enabled_flag() {
        let cfg: McuConfig =
            serde_json::from_str(r#"{"enabled":false,"white_pixel_threshold":40}"#).unwrap();
        assert_eq!(cfg.mode, McuMode::Off);
        assert_eq!(cfg.white_pixel_threshold, 40);
    }

    #[test]
    fn status_snapshot_reflects_nfc_config() {
        let status = McuStatus::from_parts(
            McuConfig {
                mode: McuMode::Nfc,
                white_pixel_threshold: 50,
            },
            McuRuntime {
                active_mode: McuMode::Nfc,
                switching: false,
            },
        );
        assert_eq!(status.config.mode, McuMode::Nfc);
        assert_eq!(status.active_mode, McuMode::Nfc);
    }
}
