import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Schematic } from "./Schematic";
import {
  ALL_BUTTONS,
  BUTTON_LABELS,
  ButtonMapping,
  JoyConButton,
  TriggerMode,
} from "./types";

interface Props {
  onDone: () => void;
  onCancel: () => void;
}

interface PresetSummary {
  id: string;
  name: string;
  description: string;
}

type Step =
  | { kind: "select_preset" }
  | {
      kind: "capture";
      queue: { action: string; suggested: JoyConButton; mode: TriggerMode }[];
      idx: number;
      result: ButtonMapping[];
    }
  | { kind: "preview"; mappings: ButtonMapping[] };

export const JoyConWizard: React.FC<Props> = ({ onDone, onCancel }) => {
  const [step, setStep] = useState<Step>({ kind: "select_preset" });
  const [presets, setPresets] = useState<PresetSummary[]>([]);
  const [activeBtns, setActiveBtns] = useState<JoyConButton[]>([]);

  useEffect(() => {
    invoke<PresetSummary[]>("plugin:joycon|joycon_list_presets")
      .then(setPresets)
      .catch(() => setPresets([]));
  }, []);

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      const u = await listen<{ button: JoyConButton; pressed: boolean }>(
        "joycon://button",
        (e) => {
          if (!alive) return;
          if (!e.payload.pressed) return;
          setActiveBtns([e.payload.button]);
          setTimeout(() => setActiveBtns([]), 250);
          setStep((prev) => {
            if (prev.kind !== "capture") return prev;
            const cur = prev.queue[prev.idx];
            if (!cur) return prev;
            const newMapping: ButtonMapping = {
              button: e.payload.button,
              payload: { kind: "builtin", id: cur.action },
              mode: cur.mode,
            };
            const result = [
              ...prev.result.filter((m) => m.button !== e.payload.button),
              newMapping,
            ];
            const nextIdx = prev.idx + 1;
            if (nextIdx >= prev.queue.length) {
              return { kind: "preview", mappings: result };
            }
            return { ...prev, idx: nextIdx, result };
          });
        },
      );
      unlisten = u;
      // start capture mode for raw button events
      await invoke("plugin:joycon|joycon_start_capture").catch(() => {});
    })();
    return () => {
      alive = false;
      unlisten?.();
      invoke("plugin:joycon|joycon_stop_capture").catch(() => {});
    };
  }, []);

  const startCustom = () => {
    const queue = [
      { action: "transcribe", suggested: "zl" as JoyConButton, mode: "hold" as TriggerMode },
      { action: "cancel", suggested: "minus" as JoyConButton, mode: "tap" as TriggerMode },
    ];
    setStep({ kind: "capture", queue, idx: 0, result: [] });
  };

  const startWithPreset = async (presetId: string) => {
    const mappings = await invoke<
      { button: JoyConButton; action: string; mode: TriggerMode }[]
    >("plugin:joycon|joycon_get_preset_mappings", { presetId });
    const queue = mappings.map((m) => ({
      action: m.action,
      suggested: m.button,
      mode: m.mode,
    }));
    setStep({ kind: "capture", queue, idx: 0, result: [] });
  };

  const skip = () => {
    setStep((prev) => {
      if (prev.kind !== "capture") return prev;
      const nextIdx = prev.idx + 1;
      if (nextIdx >= prev.queue.length) {
        return { kind: "preview", mappings: prev.result };
      }
      return { ...prev, idx: nextIdx };
    });
  };

  const save = async (mappings: ButtonMapping[]) => {
    const current = await invoke<ButtonMapping[]>(
      "plugin:joycon|joycon_get_mappings",
    );
    for (const m of current) {
      await invoke("plugin:joycon|joycon_set_mapping", {
        button: m.button,
        payload: null,
        mode: m.mode,
      });
    }
    for (const m of mappings) {
      await invoke("plugin:joycon|joycon_set_mapping", {
        button: m.button,
        payload: m.payload,
        mode: m.mode,
      });
    }
    onDone();
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">Joy-Con 设置向导</h1>
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={onCancel}
        >
          Cancel
        </button>
      </div>

      {step.kind === "select_preset" && (
        <div className="space-y-3">
          <p className="text-sm text-text-secondary">
            Choose a preset to start, or build from scratch.
          </p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {presets.map((p) => (
              <button
                key={p.id}
                onClick={() => startWithPreset(p.id)}
                className="surface-card p-4 text-left hover:border-accent/40"
              >
                <div className="font-semibold">{p.name}</div>
                <div className="text-xs text-text-secondary mt-1">
                  {p.description}
                </div>
              </button>
            ))}
            <button
              onClick={startCustom}
              className="surface-card p-4 text-left hover:border-accent/40 border-dashed"
            >
              <div className="font-semibold">Custom (blank)</div>
              <div className="text-xs text-text-secondary mt-1">
                Map buttons one by one yourself
              </div>
            </button>
          </div>
        </div>
      )}

      {step.kind === "capture" && (
        <div className="space-y-4">
          <div className="surface-card p-4">
            <div className="text-sm text-text-secondary mb-1">
              Step {step.idx + 1} of {step.queue.length}
            </div>
            <div className="text-lg font-semibold">
              Press the button you want for{" "}
              <span className="text-accent">
                {step.queue[step.idx].action}
              </span>
            </div>
            <div className="text-xs text-text-secondary mt-1">
              Suggested: {BUTTON_LABELS[step.queue[step.idx].suggested]} ·
              Mode: {step.queue[step.idx].mode}
            </div>
          </div>
          <div className="surface-card p-4 flex justify-center">
            <Schematic
              layout="pair"
              highlight={[step.queue[step.idx].suggested]}
              active={activeBtns}
              className="w-full max-w-md"
            />
          </div>
          <div className="flex justify-between">
            <button
              className="text-sm px-3 py-1.5 rounded-mac border border-border hover:border-accent"
              onClick={skip}
            >
              Skip this action
            </button>
            <button
              className="text-sm px-3 py-1.5 rounded-mac border border-border hover:border-accent"
              onClick={() =>
                setStep({ kind: "preview", mappings: step.result })
              }
            >
              Done early
            </button>
          </div>
        </div>
      )}

      {step.kind === "preview" && (
        <div className="space-y-4">
          <p className="text-sm text-text-secondary">
            Review your mappings, then save.
          </p>
          <div className="surface-card overflow-hidden">
            <table className="w-full text-sm">
              <thead className="bg-surface-2">
                <tr>
                  <th className="text-left px-3 py-2">Button</th>
                  <th className="text-left px-3 py-2">Action</th>
                  <th className="text-left px-3 py-2">Mode</th>
                </tr>
              </thead>
              <tbody>
                {step.mappings.map((m) => (
                  <tr key={m.button} className="border-t border-border">
                    <td className="px-3 py-2 font-mono text-xs">
                      {BUTTON_LABELS[m.button]}
                    </td>
                    <td className="px-3 py-2">
                      {m.payload?.kind === "builtin" ? m.payload.id : "—"}
                    </td>
                    <td className="px-3 py-2">{m.mode}</td>
                  </tr>
                ))}
                {step.mappings.length === 0 && (
                  <tr>
                    <td colSpan={3} className="px-3 py-4 text-center text-text-secondary text-xs">
                      No mappings recorded
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
          <div className="flex justify-end gap-2">
            <button
              className="text-sm px-3 py-1.5 rounded-mac border border-border"
              onClick={onCancel}
            >
              Cancel
            </button>
            <button
              className="text-sm px-3 py-1.5 rounded-mac bg-accent text-white hover:bg-accent-hover"
              onClick={() => save(step.mappings)}
            >
              Save Mappings
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
