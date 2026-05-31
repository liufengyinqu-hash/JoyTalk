//! Action injection: keyboard chords + text snippets via enigo.

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use log::{info, warn};
use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::types::{KeyChord, Modifier};

// Global enigo instance — Enigo::new() takes ~5-15ms on macOS,
// caching avoids that overhead per keystroke.
static ENIGO: Lazy<Mutex<Option<Enigo>>> =
    Lazy::new(|| Mutex::new(Enigo::new(&Settings::default()).ok()));

fn with_enigo<F: FnOnce(&mut Enigo)>(f: F) {
    let mut guard = match ENIGO.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if guard.is_none() {
        *guard = Enigo::new(&Settings::default()).ok();
    }
    if let Some(eg) = guard.as_mut() {
        f(eg);
    } else {
        warn!("[joycon] enigo unavailable");
    }
}

pub fn inject_keyboard(chords: &[KeyChord]) {
    with_enigo(|eg| {
        for chord in chords {
            if let Err(e) = press_chord(eg, chord) {
                warn!("[joycon] chord injection failed: {e}");
            }
        }
    });
}

pub fn inject_text(text: &str) {
    with_enigo(|eg| {
        if let Err(e) = eg.text(text) {
            warn!("[joycon] text injection failed: {e:?}");
        }
    });
}

pub fn open_app(bundle_id: &str) {
    if bundle_id.trim().is_empty() {
        return;
    }
    if bundle_id.contains('\n') || bundle_id.contains('\0') || bundle_id.len() > 256 {
        warn!("[joycon] reject suspicious bundle_id: {bundle_id:?}");
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let arg = if bundle_id.contains('.') {
            vec!["-b".to_string(), bundle_id.to_string()]
        } else {
            vec!["-a".to_string(), bundle_id.to_string()]
        };
        match std::process::Command::new("open").args(&arg).spawn() {
            Ok(_) => info!("[joycon] open app: {bundle_id}"),
            Err(e) => warn!("[joycon] open app failed: {e}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        warn!("[joycon] OpenApp only supported on macOS currently");
    }
}

pub fn run_shell(command: &str) {
    if command.trim().is_empty() {
        return;
    }
    if command.len() > 4096 {
        warn!("[joycon] reject overlong shell command");
        return;
    }
    match std::process::Command::new("sh")
        .args(["-c", command])
        .spawn()
    {
        Ok(_) => info!("[joycon] shell: {command}"),
        Err(e) => warn!("[joycon] shell failed: {e}"),
    }
}

pub fn run_applescript(script: &str) {
    if script.trim().is_empty() {
        return;
    }
    if script.len() > 8192 {
        warn!("[joycon] reject overlong applescript");
        return;
    }
    #[cfg(target_os = "macos")]
    {
        match std::process::Command::new("osascript")
            .args(["-e", script])
            .spawn()
        {
            Ok(_) => info!("[joycon] applescript fired"),
            Err(e) => warn!("[joycon] applescript failed: {e}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        warn!("[joycon] AppleScript only supported on macOS");
    }
}

fn press_chord(eg: &mut Enigo, chord: &KeyChord) -> Result<(), String> {
    let mod_keys: Vec<Key> = chord.modifiers.iter().map(modifier_to_key).collect();
    for k in &mod_keys {
        eg.key(*k, Direction::Press).map_err(|e| format!("{e:?}"))?;
    }
    let main_key = parse_key(&chord.key);
    if let Some(k) = main_key {
        eg.key(k, Direction::Click).map_err(|e| format!("{e:?}"))?;
    }
    for k in mod_keys.iter().rev() {
        eg.key(*k, Direction::Release)
            .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}

fn modifier_to_key(m: &Modifier) -> Key {
    match m {
        Modifier::Cmd => Key::Meta,
        Modifier::Ctrl => Key::Control,
        Modifier::Alt => Key::Alt,
        Modifier::Shift => Key::Shift,
    }
}

fn parse_key(s: &str) -> Option<Key> {
    match s {
        "Enter" | "Return" => Some(Key::Return),
        "Tab" => Some(Key::Tab),
        "Escape" | "Esc" => Some(Key::Escape),
        "Backspace" => Some(Key::Backspace),
        "Delete" => Some(Key::Delete),
        "Space" | " " => Some(Key::Space),
        "ArrowUp" | "Up" => Some(Key::UpArrow),
        "ArrowDown" | "Down" => Some(Key::DownArrow),
        "ArrowLeft" | "Left" => Some(Key::LeftArrow),
        "ArrowRight" | "Right" => Some(Key::RightArrow),
        "Home" => Some(Key::Home),
        "End" => Some(Key::End),
        "PageUp" => Some(Key::PageUp),
        "PageDown" => Some(Key::PageDown),
        s if s.len() == 1 => s.chars().next().map(Key::Unicode),
        s if s.starts_with('F') && s.len() <= 3 => {
            s[1..].parse::<u32>().ok().and_then(|n| match n {
                1 => Some(Key::F1),
                2 => Some(Key::F2),
                3 => Some(Key::F3),
                4 => Some(Key::F4),
                5 => Some(Key::F5),
                6 => Some(Key::F6),
                7 => Some(Key::F7),
                8 => Some(Key::F8),
                9 => Some(Key::F9),
                10 => Some(Key::F10),
                11 => Some(Key::F11),
                12 => Some(Key::F12),
                _ => None,
            })
        }
        _ => None,
    }
}
