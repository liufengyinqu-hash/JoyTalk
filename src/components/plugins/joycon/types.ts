// Mirror of Rust JoyConButton enum string-form
export type JoyConButton =
  | "a"
  | "b"
  | "x"
  | "y"
  | "plus"
  | "minus"
  | "home"
  | "capture"
  | "l"
  | "r"
  | "zl"
  | "zr"
  | "l_stick"
  | "r_stick"
  | "sl_left"
  | "sr_left"
  | "sl_right"
  | "sr_right"
  | "up"
  | "down"
  | "left"
  | "right"
  | "l_stick_up"
  | "l_stick_down"
  | "l_stick_left"
  | "l_stick_right"
  | "r_stick_up"
  | "r_stick_down"
  | "r_stick_left"
  | "r_stick_right"
  | "shake"
  | "flip_up"
  | "flip_down"
  | "tilt_left"
  | "tilt_right"
  | "shake_horizontal"
  | "shake_vertical"
  | "ir_proximity"
  | "nfc_tag_present";

export type TriggerMode = "hold" | "tap" | "double_tap" | "long_press" | "repeat";

export type ControllerKind =
  | "joy_con_left"
  | "joy_con_right"
  | "pro_controller"
  | "unknown";

export interface KeyChord {
  modifiers: ("cmd" | "ctrl" | "alt" | "shift")[];
  key: string;
}

export type ActionPayload =
  | { kind: "builtin"; id: string }
  | { kind: "keyboard"; chords: KeyChord[] }
  | { kind: "text"; text: string }
  | { kind: "open_app"; bundle_id: string }
  | { kind: "shell"; command: string }
  | { kind: "apple_script"; script: string };

export interface ButtonMapping {
  button: JoyConButton;
  payload: ActionPayload | null;
  mode: TriggerMode;
}

export interface ConnectedController {
  kind: ControllerKind;
  serial: string;
}

export interface JoyConStatus {
  connected: boolean;
  battery: number;
  charging: boolean;
  device_count: number;
  connected_controllers?: ConnectedController[];
}

export interface ControllerDetected {
  kind: ControllerKind;
  serial: string;
  device_index: number;
  is_first_pair: boolean;
}

export interface Macro {
  id: string;
  name: string;
  payload: ActionPayload;
}

export interface PresetMapping {
  button: JoyConButton;
  action: string;
  mode: TriggerMode;
}

export interface Preset {
  id: string;
  name: string;
  description: string;
  kind: ControllerKind | "any";
  mappings: PresetMapping[];
}

export const ALL_BUTTONS: JoyConButton[] = [
  "a", "b", "x", "y",
  "plus", "minus", "home", "capture",
  "l", "r", "zl", "zr",
  "l_stick", "r_stick",
  "sl_left", "sr_left", "sl_right", "sr_right",
  "up", "down", "left", "right",
  "l_stick_up", "l_stick_down", "l_stick_left", "l_stick_right",
  "r_stick_up", "r_stick_down", "r_stick_left", "r_stick_right",
  "shake", "flip_up", "flip_down",
  "tilt_left", "tilt_right",
  "shake_horizontal", "shake_vertical",
  "ir_proximity",
  "nfc_tag_present",
];

export const BUTTON_LABELS: Record<JoyConButton, string> = {
  a: "A", b: "B", x: "X", y: "Y",
  plus: "+", minus: "−", home: "Home", capture: "Capture",
  l: "L", r: "R", zl: "ZL", zr: "ZR",
  l_stick: "L 摇杆", r_stick: "R 摇杆",
  sl_left: "SL (L)", sr_left: "SR (L)",
  sl_right: "SL (R)", sr_right: "SR (R)",
  up: "↑", down: "↓", left: "←", right: "→",
  l_stick_up: "L↑", l_stick_down: "L↓",
  l_stick_left: "L←", l_stick_right: "L→",
  r_stick_up: "R↑", r_stick_down: "R↓",
  r_stick_left: "R←", r_stick_right: "R→",
  shake: "🤲 摇晃",
  flip_up: "🔼 上翻",
  flip_down: "🔽 下翻",
  tilt_left: "↺ 左倾",
  tilt_right: "↻ 右倾",
  shake_horizontal: "↔ 横晃",
  shake_vertical: "↕ 竖晃",
  ir_proximity: "📡 IR 接近",
  nfc_tag_present: "🏷 NFC 标签",
};

export const MODE_LABELS: Record<TriggerMode, string> = {
  hold: "按住",
  tap: "点按",
  double_tap: "双击",
  long_press: "长按",
  repeat: "连发 (长按重复)",
};
