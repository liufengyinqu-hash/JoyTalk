import React, { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ALL_BUTTONS,
  BUTTON_LABELS,
  JoyConButton,
  KeyChord,
  TriggerMode,
} from "./types";

interface Props {
  onClose: () => void;
}

const MOD_DISPLAY: Record<string, string> = {
  cmd: "⌘",
  ctrl: "⌃",
  alt: "⌥",
  shift: "⇧",
};

export const MacroRecorder: React.FC<Props> = ({ onClose }) => {
  const [type, setType] = useState<"keyboard" | "text">("keyboard");
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [chords, setChords] = useState<KeyChord[]>([]);
  const [recording, setRecording] = useState(false);
  const [targetButton, setTargetButton] = useState<JoyConButton>("a");
  const [mode, setMode] = useState<TriggerMode>("tap");
  const captureRef = useRef<HTMLDivElement>(null);

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
      if (key === "Meta" || key === "Control" || key === "Alt" || key === "Shift")
        return;
      setChords((prev) => [...prev, { modifiers: mods, key }]);
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [recording]);

  const formatChord = (c: KeyChord) =>
    [...c.modifiers.map((m) => MOD_DISPLAY[m] ?? m), c.key].join(" ");

  const save = async () => {
    if (!name.trim()) {
      alert("Macro needs a name");
      return;
    }
    const payload =
      type === "keyboard"
        ? { kind: "keyboard", chords }
        : { kind: "text", text };
    await invoke("plugin:joycon|joycon_set_mapping", {
      button: targetButton,
      payload,
      mode,
    });
    onClose();
  };

  return (
    <div className="max-w-2xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">Record Macro</h1>
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={onClose}
        >
          ← Back
        </button>
      </div>

      <div className="surface-card p-4 space-y-4">
        <div className="space-y-2">
          <label className="text-sm font-medium">Name</label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Save and switch tab"
            className="w-full px-3 py-2 text-sm bg-surface-2 border border-border rounded-mac"
          />
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">Type</label>
          <div className="flex gap-2">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                checked={type === "keyboard"}
                onChange={() => setType("keyboard")}
              />
              Keyboard shortcut
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                checked={type === "text"}
                onChange={() => setType("text")}
              />
              Text snippet
            </label>
          </div>
        </div>

        {type === "keyboard" ? (
          <div className="space-y-2">
            <label className="text-sm font-medium">Recorded keys</label>
            <div
              ref={captureRef}
              className="min-h-[60px] surface-2 border border-border rounded-mac p-3 flex flex-wrap gap-2 items-center"
            >
              {chords.length === 0 && (
                <span className="text-xs text-text-secondary">
                  {recording ? "Press keys…" : "Click Start to record"}
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
                className={`text-sm px-3 py-1.5 rounded-mac ${recording ? "bg-danger text-white" : "bg-accent text-white"}`}
                onClick={() => setRecording((v) => !v)}
              >
                {recording ? "Stop" : "Start Recording"}
              </button>
              <button
                className="text-sm px-3 py-1.5 rounded-mac border border-border"
                onClick={() => setChords([])}
              >
                Clear
              </button>
            </div>
          </div>
        ) : (
          <div className="space-y-2">
            <label className="text-sm font-medium">Text to inject</label>
            <textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              rows={3}
              className="w-full px-3 py-2 text-sm bg-surface-2 border border-border rounded-mac font-mono"
              placeholder="Hello 你好"
            />
          </div>
        )}

        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-2">
            <label className="text-sm font-medium">Trigger Button</label>
            <select
              className="w-full bg-surface-2 border border-border rounded px-2 py-1.5 text-sm"
              value={targetButton}
              onChange={(e) => setTargetButton(e.target.value as JoyConButton)}
            >
              {ALL_BUTTONS.map((b) => (
                <option key={b} value={b}>
                  {BUTTON_LABELS[b]}
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Mode</label>
            <select
              className="w-full bg-surface-2 border border-border rounded px-2 py-1.5 text-sm"
              value={mode}
              onChange={(e) => setMode(e.target.value as TriggerMode)}
            >
              <option value="tap">Tap</option>
              <option value="hold">Hold</option>
              <option value="double_tap">Double Tap</option>
              <option value="long_press">Long Press</option>
            </select>
          </div>
        </div>
      </div>

      <div className="flex justify-end gap-2">
        <button
          className="text-sm px-3 py-1.5 rounded-mac border border-border"
          onClick={onClose}
        >
          Cancel
        </button>
        <button
          className="text-sm px-3 py-1.5 rounded-mac bg-accent text-white hover:bg-accent-hover"
          onClick={save}
        >
          Save Macro
        </button>
      </div>
    </div>
  );
};
