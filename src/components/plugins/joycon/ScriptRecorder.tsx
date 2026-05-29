import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ALL_BUTTONS,
  BUTTON_LABELS,
  JoyConButton,
  TriggerMode,
} from "./types";

interface Props {
  onClose: () => void;
}

export const ScriptRecorder: React.FC<Props> = ({ onClose }) => {
  const [type, setType] = useState<"shell" | "apple_script">("shell");
  const [body, setBody] = useState("");
  const [targetButton, setTargetButton] = useState<JoyConButton>("a");
  const [mode, setMode] = useState<TriggerMode>("tap");

  const save = async () => {
    if (!body.trim()) {
      alert("Empty script");
      return;
    }
    const payload =
      type === "shell"
        ? { kind: "shell", command: body }
        : { kind: "apple_script", script: body };
    await invoke("plugin:joycon|joycon_set_mapping", {
      button: targetButton,
      payload,
      mode,
    });
    onClose();
  };

  const placeholder =
    type === "shell"
      ? 'osascript -e \'tell app "Spotify" to playpause\''
      : 'tell application "Spotify" to playpause';

  return (
    <div className="max-w-2xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">Record Script</h1>
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={onClose}
        >
          ← Back
        </button>
      </div>

      <div className="surface-card p-4 space-y-4">
        <div className="space-y-2">
          <label className="text-sm font-medium">Type</label>
          <div className="flex gap-3">
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                checked={type === "shell"}
                onChange={() => setType("shell")}
              />
              Shell command
            </label>
            <label className="flex items-center gap-2 text-sm">
              <input
                type="radio"
                checked={type === "apple_script"}
                onChange={() => setType("apple_script")}
              />
              AppleScript
            </label>
          </div>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">
            {type === "shell" ? "Shell command" : "AppleScript"}
          </label>
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            rows={5}
            className="w-full px-3 py-2 text-sm bg-surface-2 border border-border rounded-mac font-mono"
            placeholder={placeholder}
          />
          <p className="text-xs text-text-secondary">
            {type === "shell"
              ? "Runs via sh -c. Limit 4KB. You are responsible for what runs."
              : "Runs via osascript -e. macOS only. Limit 8KB."}
          </p>
        </div>

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
          Save Script
        </button>
      </div>
    </div>
  );
};
