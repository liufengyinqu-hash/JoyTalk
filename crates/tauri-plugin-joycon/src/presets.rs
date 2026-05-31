//! Built-in preset library (embedded JSON).

use std::collections::HashMap;

use crate::types::{Preset, PresetSummary};

const VIBE_CODING: &str = include_str!("../presets/vibe-coding.json");
const VIDEO_EDITING: &str = include_str!("../presets/video-editing.json");
const MEETING_MUTE: &str = include_str!("../presets/meeting-mute.json");
const DEFAULT: &str = include_str!("../presets/default.json");
const BLANK: &str = include_str!("../presets/blank.json");
const TWO_BUTTON: &str = include_str!("../presets/two-button-transcribe.json");

pub fn builtin_presets() -> Vec<Preset> {
    let raw = [
        VIBE_CODING,
        TWO_BUTTON,
        VIDEO_EDITING,
        MEETING_MUTE,
        DEFAULT,
        BLANK,
    ];
    raw.iter()
        .filter_map(|s| serde_json::from_str::<Preset>(s).ok())
        .collect()
}

pub fn builtin_summaries() -> Vec<PresetSummary> {
    builtin_presets()
        .into_iter()
        .map(|p| PresetSummary {
            id: p.id,
            name: p.name,
            description: p.description,
        })
        .collect()
}

pub fn find_builtin(id: &str) -> Option<Preset> {
    builtin_presets().into_iter().find(|p| p.id == id)
}

/// Validate parsed preset before applying. Returns Err with reason if rejected.
pub fn validate(preset: &Preset) -> Result<(), String> {
    if preset.id.trim().is_empty() {
        return Err("preset id is empty".into());
    }
    if preset.id.len() > 64 {
        return Err("preset id too long".into());
    }
    if preset.mappings.len() > 64 {
        return Err("too many mappings".into());
    }
    let mut seen = HashMap::new();
    for m in &preset.mappings {
        if seen.insert(m.button, ()).is_some() {
            return Err(format!("duplicate button: {:?}", m.button));
        }
        if m.action.len() > 64 {
            return Err("action id too long".into());
        }
    }
    Ok(())
}
