import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ALL_BUTTONS,
  ActionPayload,
  BUTTON_LABELS,
  ButtonMapping,
  JoyConButton,
  KeyChord,
  TriggerMode,
} from "../../plugins/joycon/types";

type ActionKind =
  | "builtin"
  | "keyboard"
  | "text"
  | "open_app"
  | "shell"
  | "apple_script";

interface AppEntry {
  name: string;
  bundle_id: string;
  path: string;
}

interface Props {
  open: boolean;
  initial?: ButtonMapping | null;
  onClose: () => void;
  onSaved: () => void;
}

const MOD_DISPLAY: Record<string, string> = {
  cmd: "⌘",
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
};

export const BindingEditor: React.FC<Props> = ({
  open,
  initial,
  onClose,
  onSaved,
}) => {
  const [button, setButton] = useState<JoyConButton>("a");
  const [mode, setMode] = useState<TriggerMode>("tap");
  const [kind, setKind] = useState<ActionKind>("builtin");
  const [actions, setActions] = useState<string[]>([]);
  const [builtinId, setBuiltinId] = useState("transcribe");
  const [text, setText] = useState("");
  const [shellCmd, setShellCmd] = useState("");
  const [asScript, setAsScript] = useState("");
  const [chords, setChords] = useState<KeyChord[]>([]);
  const [recording, setRecording] = useState(false);
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [bundleId, setBundleId] = useState("");

  useEffect(() => {
    if (!open) return;
    invoke<string[]>("plugin:joycon|joycon_list_actions")
      .then(setActions)
      .catch(() => setActions([]));
    invoke<AppEntry[]>("plugin:joycon|joycon_list_apps")
      .then(setApps)
      .catch(() => setApps([]));

    if (initial) {
      setButton(initial.button);
      setMode(initial.mode);
      const p = initial.payload;
      if (!p) {
        setKind("builtin");
      } else if (p.kind === "builtin") {
        setKind("builtin");
        setBuiltinId(p.id);
      } else if (p.kind === "keyboard") {
        setKind("keyboard");
        setChords(p.chords);
      } else if (p.kind === "text") {
        setKind("text");
        setText(p.text);
      } else if (p.kind === "open_app") {
        setKind("open_app");
        setBundleId(p.bundle_id);
      } else if (p.kind === "shell") {
        setKind("shell");
        setShellCmd(p.command);
      } else if (p.kind === "apple_script") {
        setKind("apple_script");
        setAsScript(p.script);
      }
    } else {
      setButton("a");
      setMode("tap");
      setKind("builtin");
      setBuiltinId("transcribe");
      setText("");
      setShellCmd("");
      setAsScript("");
      setChords([]);
      setBundleId("");
    }
  }, [open, initial]);

  useEffect(() => {
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      const mods: KeyChord["modifiers"] = [];
      if (e.metaKey) mods.push("cmd");
      if (e.ctrlKey) mods.push("ctrl");
      if (e.altKey) mods.push("alt");
      if (e.shiftKey) mods.push("shift");
      const key = e.key;
      if (
        key === "Meta" ||
        key === "Control" ||
        key === "Alt" ||
        key === "Shift"
      )
        return;
      setChords((prev) => [...prev, { modifiers: mods, key }]);
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [recording]);

  // listen for raw JC button presses to allow capturing button choice
  const [capturing, setCapturing] = useState(false);
  useEffect(() => {
    if (!open || !capturing) return;
    let alive = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      const u = await listen<{ button: JoyConButton; pressed: boolean }>(
        "joycon://button",
        (e) => {
          if (!alive || !e.payload.pressed) return;
          setButton(e.payload.button);
          setCapturing(false);
          invoke("plugin:joycon|joycon_stop_capture").catch(() => {});
        },
      );
      unlisten = u;
      await invoke("plugin:joycon|joycon_start_capture").catch(() => {});
    })();
    return () => {
      alive = false;
      unlisten?.();
      invoke("plugin:joycon|joycon_stop_capture").catch(() => {});
    };
  }, [open, capturing]);

  const formatChord = (c: KeyChord) =>
    [...c.modifiers.map((m) => MOD_DISPLAY[m] ?? m), c.key].join(" ");

  const buildPayload = (): ActionPayload | null => {
    switch (kind) {
      case "builtin":
        return builtinId ? { kind: "builtin", id: builtinId } : null;
      case "keyboard":
        return chords.length > 0 ? { kind: "keyboard", chords } : null;
      case "text":
        return text ? { kind: "text", text } : null;
      case "open_app":
        return bundleId ? { kind: "open_app", bundle_id: bundleId } : null;
      case "shell":
        return shellCmd ? { kind: "shell", command: shellCmd } : null;
      case "apple_script":
        return asScript ? { kind: "apple_script", script: asScript } : null;
    }
  };

  const save = async () => {
    const payload = buildPayload();
    await invoke("plugin:joycon|joycon_set_mapping", {
      button,
      payload,
      mode,
    });
    onSaved();
    onClose();
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="surface-card max-w-xl w-full max-h-[85vh] overflow-y-auto p-4 space-y-4"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">
            {initial ? "编辑映射" : "新增映射"}
          </h2>
          <button
            className="text-xs text-text-secondary hover:text-text"
            onClick={onClose}
          >
            ✕
          </button>
        </div>

        {/* Button + mode */}
        <div className="grid grid-cols-2 gap-3">
          <div>
            <label className="text-xs font-medium">触发按键</label>
            <div className="flex gap-1 mt-1">
              <select
                className="flex-1 bg-surface-2 border border-border rounded px-2 py-1.5 text-sm"
                value={button}
                onChange={(e) => setButton(e.target.value as JoyConButton)}
              >
                {ALL_BUTTONS.map((b) => (
                  <option key={b} value={b}>
                    {BUTTON_LABELS[b]}
                  </option>
                ))}
              </select>
              <button
                className={`text-xs px-2 rounded border ${capturing ? "border-danger text-danger" : "border-border hover:border-accent"}`}
                onClick={() => setCapturing((v) => !v)}
              >
                {capturing ? "取消" : "按 JC"}
              </button>
            </div>
          </div>
          <div>
            <label className="text-xs font-medium">触发模式</label>
            <select
              className="w-full mt-1 bg-surface-2 border border-border rounded px-2 py-1.5 text-sm"
              value={mode}
              onChange={(e) => setMode(e.target.value as TriggerMode)}
            >
              <option value="tap">Tap (点按)</option>
              <option value="hold">Hold (按住)</option>
              <option value="double_tap">Double Tap (双击)</option>
              <option value="long_press">Long Press (长按)</option>
              <option value="repeat">Repeat (连发)</option>
            </select>
          </div>
        </div>

        {/* Action kind */}
        <div>
          <label className="text-xs font-medium">动作类型</label>
          <div className="grid grid-cols-3 gap-1 mt-1">
            {(
              [
                ["builtin", "内置"],
                ["keyboard", "键盘宏"],
                ["text", "文本"],
                ["open_app", "打开 App"],
                ["shell", "Shell"],
                ["apple_script", "AppleScript"],
              ] as [ActionKind, string][]
            ).map(([k, label]) => (
              <button
                key={k}
                onClick={() => setKind(k)}
                className={`text-xs py-1 rounded border ${
                  kind === k
                    ? "bg-accent text-white border-accent"
                    : "border-border hover:border-accent"
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        {/* Per-kind editor */}
        {kind === "builtin" && (
          <div>
            <label className="text-xs font-medium">内置动作</label>
            <select
              className="w-full mt-1 bg-surface-2 border border-border rounded px-2 py-1.5 text-sm"
              value={builtinId}
              onChange={(e) => setBuiltinId(e.target.value)}
            >
              {actions.map((a) => (
                <option key={a} value={a}>
                  {a}
                </option>
              ))}
            </select>
            <p className="text-xs text-text-secondary mt-1">
              transcribe / start_transcribe / stop_transcribe / cancel
            </p>
          </div>
        )}

        {kind === "keyboard" && (
          <div className="space-y-2">
            <label className="text-xs font-medium">键盘快捷键序列</label>
            <div className="surface-2 border border-border rounded-mac p-3 min-h-[60px] flex flex-wrap gap-2 items-center">
              {chords.length === 0 && (
                <span className="text-xs text-text-secondary">
                  {recording ? "请按键盘…" : '点 "录制" 开始'}
                </span>
              )}
              {chords.map((c, i) => (
                <span
                  key={i}
                  className="px-2 py-0.5 rounded bg-accent/15 text-accent text-xs font-mono"
                >
                  {formatChord(c)}
                </span>
              ))}
            </div>
            <div className="flex gap-2">
              <button
                className={`text-xs px-2 py-1 rounded ${recording ? "bg-danger text-white" : "bg-accent text-white"}`}
                onClick={() => setRecording((v) => !v)}
              >
                {recording ? "停止录制" : "开始录制"}
              </button>
              <button
                className="text-xs px-2 py-1 rounded border border-border"
                onClick={() => setChords([])}
              >
                清空
              </button>
            </div>
            <p className="text-xs text-text-secondary">
              例: ⌘+S, ⌘+Tab, ⌘+Shift+P
            </p>
          </div>
        )}

        {kind === "text" && (
          <div>
            <label className="text-xs font-medium">文本片段</label>
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={3}
              className="w-full mt-1 px-2 py-1.5 text-sm bg-surface-2 border border-border rounded font-mono"
              placeholder="例: /voice"
            />
          </div>
        )}

        {kind === "open_app" && (
          <div>
            <label className="text-xs font-medium">选择 App</label>
            <select
              className="w-full mt-1 bg-surface-2 border border-border rounded px-2 py-1.5 text-sm"
              value={bundleId}
              onChange={(e) => setBundleId(e.target.value)}
            >
              <option value="">— 请选择 —</option>
              {apps.map((a) => (
                <option key={a.path} value={a.bundle_id || a.name}>
                  {a.name} ({a.bundle_id || a.path})
                </option>
              ))}
            </select>
          </div>
        )}

        {kind === "shell" && (
          <div>
            <label className="text-xs font-medium">Shell 命令</label>
            <textarea
              value={shellCmd}
              onChange={(e) => setShellCmd(e.target.value)}
              rows={3}
              className="w-full mt-1 px-2 py-1.5 text-sm bg-surface-2 border border-border rounded font-mono"
              placeholder='例: open -a "Spotify"'
            />
            <p className="text-xs text-text-secondary mt-1">
              通过 sh -c 执行, 上限 4KB, 自负安全责任
            </p>
          </div>
        )}

        {kind === "apple_script" && (
          <div>
            <label className="text-xs font-medium">AppleScript</label>
            <textarea
              value={asScript}
              onChange={(e) => setAsScript(e.target.value)}
              rows={4}
              className="w-full mt-1 px-2 py-1.5 text-sm bg-surface-2 border border-border rounded font-mono"
              placeholder='例: tell application "Spotify" to playpause'
            />
            <p className="text-xs text-text-secondary mt-1">
              通过 osascript -e 执行, macOS 限定, 上限 8KB
            </p>
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2 pt-2 border-t border-border">
          <button
            className="text-sm px-3 py-1.5 rounded-mac border border-border"
            onClick={onClose}
          >
            取消
          </button>
          <button
            className="text-sm px-3 py-1.5 rounded-mac bg-accent text-white hover:bg-accent-hover"
            onClick={save}
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
};
