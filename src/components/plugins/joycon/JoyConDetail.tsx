import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Schematic, SchematicLayout } from "./Schematic";
import {
  ALL_BUTTONS,
  BUTTON_LABELS,
  ButtonMapping,
  ControllerKind,
  JoyConButton,
  JoyConStatus,
  MODE_LABELS,
  TriggerMode,
} from "./types";

interface AppEntry {
  name: string;
  bundle_id: string;
  path: string;
}

interface Props {
  onLaunchWizard: () => void;
}

const KIND_TO_LAYOUT: Record<ControllerKind, SchematicLayout> = {
  joy_con_left: "left",
  joy_con_right: "right",
  pro_controller: "pro",
  unknown: "pair",
};

const CONTROLLER_NAMES: Record<ControllerKind, string> = {
  joy_con_left: "Joy-Con (L)",
  joy_con_right: "Joy-Con (R)",
  pro_controller: "Pro Controller",
  unknown: "Controller",
};

function resolveConnectedControllers(
  status: JoyConStatus,
  controllers: { kind: ControllerKind; serial: string }[],
): { kind: ControllerKind; serial: string }[] {
  if (status.connected_controllers && status.connected_controllers.length > 0) {
    return status.connected_controllers;
  }
  return controllers;
}

export const JoyConDetail: React.FC<Props> = ({ onLaunchWizard }) => {
  const [enabled, setEnabled] = useState(true);
  const [mappings, setMappings] = useState<ButtonMapping[]>([]);
  const [actions, setActions] = useState<string[]>([]);
  const [status, setStatus] = useState<JoyConStatus>({
    connected: false,
    battery: 0,
    charging: false,
    device_count: 0,
  });
  const [activeBtns, setActiveBtns] = useState<JoyConButton[]>([]);
  const [layout, setLayout] = useState<SchematicLayout>("pair");
  const [controllers, setControllers] = useState<
    { kind: ControllerKind; serial: string }[]
  >([]);
  const [presets, setPresets] = useState<{ id: string; name: string; description: string }[]>([]);
  const [showImport, setShowImport] = useState(false);
  const [importUrl, setImportUrl] = useState("");
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [appPickerFor, setAppPickerFor] = useState<JoyConButton | null>(null);

  const reload = async () => {
    try {
      const [en, mp, ac, st, ps, appList] = await Promise.all([
        invoke<boolean>("plugin:joycon|joycon_get_enabled"),
        invoke<ButtonMapping[]>("plugin:joycon|joycon_get_mappings"),
        invoke<string[]>("plugin:joycon|joycon_list_actions"),
        invoke<JoyConStatus>("plugin:joycon|joycon_get_status"),
        invoke<{ id: string; name: string; description: string }[]>(
          "plugin:joycon|joycon_list_presets",
        ).catch(() => []),
        invoke<AppEntry[]>("plugin:joycon|joycon_list_apps").catch(() => []),
      ]);
      setEnabled(en);
      setMappings(mp);
      setActions(ac);
      setStatus(st);
      setPresets(ps);
      setApps(appList);
      const known = resolveConnectedControllers(st, []);
      if (known.length === 1) {
        setLayout(KIND_TO_LAYOUT[known[0].kind] ?? "pair");
      }
      if (known.length > 0) {
        setControllers(known);
      }
    } catch (e) {
      console.warn("[joycon] reload failed", e);
    }
  };

  useEffect(() => {
    let alive = true;
    const unlisteners: Array<() => void> = [];
    (async () => {
      const u1 = await listen<JoyConStatus>("joycon://status", (e) => {
        if (!alive) return;
        setStatus(e.payload);
        const known = resolveConnectedControllers(e.payload, []);
        if (known.length === 1) {
          setLayout(KIND_TO_LAYOUT[known[0].kind] ?? "pair");
        }
        if (known.length > 0) {
          setControllers(known);
        }
      });
      unlisteners.push(u1);
      const u2 = await listen<{ button: JoyConButton; pressed: boolean }>(
        "joycon://button",
        (e) => {
          if (!alive) return;
          setActiveBtns((prev) => {
            const next = new Set(prev);
            if (e.payload.pressed) {
              next.add(e.payload.button);
            } else {
              next.delete(e.payload.button);
            }
            return Array.from(next);
          });
        },
      );
      unlisteners.push(u2);
      const u3 = await listen<{ kind: ControllerKind; serial: string }>(
        "joycon://controller_detected",
        (e) => {
          if (!alive) return;
          setLayout(KIND_TO_LAYOUT[e.payload.kind] ?? "pair");
          setControllers((prev) => [
            ...prev.filter((c) => c.serial !== e.payload.serial),
            { kind: e.payload.kind, serial: e.payload.serial },
          ]);
        },
      );
      unlisteners.push(u3);
      // also try to auto-detect once on mount based on connection state — best effort
      await reload();
    })();
    return () => {
      alive = false;
      unlisteners.forEach((u) => u());
    };
  }, []);

  const labels = mappings.reduce<Partial<Record<JoyConButton, string>>>(
    (acc, m) => {
      if (!m.payload) return acc;
      const name =
        m.payload.kind === "builtin"
          ? m.payload.id
          : m.payload.kind === "keyboard"
            ? "macro"
            : m.payload.kind === "text"
              ? "text"
              : m.payload.kind === "open_app"
                ? `→ ${m.payload.bundle_id.split(".").pop() ?? "app"}`
                : "?";
      acc[m.button] = name;
      return acc;
    },
    {},
  );

  const highlight = mappings
    .filter((m) => m.payload != null)
    .map((m) => m.button);

  const handleSetEnabled = async (v: boolean) => {
    await invoke("plugin:joycon|joycon_set_enabled", { enabled: v });
    setEnabled(v);
  };

  const handleSetAction = async (
    button: JoyConButton,
    actionId: string | null,
    mode: TriggerMode,
  ) => {
    const payload = actionId ? { kind: "builtin", id: actionId } : null;
    await invoke("plugin:joycon|joycon_set_mapping", { button, payload, mode });
    await reload();
  };

  const handleReset = async () => {
    await invoke("plugin:joycon|joycon_reset_mappings");
    await reload();
  };

  const handleLoadPreset = async (id: string) => {
    await invoke("plugin:joycon|joycon_load_preset", { presetId: id });
    await reload();
  };

  const handleExport = async () => {
    const json = await invoke<string>("plugin:joycon|joycon_export_mappings");
    await navigator.clipboard.writeText(json);
    alert("Mapping JSON copied to clipboard");
  };

  const handleImportUrl = async () => {
    if (!importUrl.trim()) return;
    try {
      await invoke("plugin:joycon|joycon_load_preset_from_url", {
        url: importUrl.trim(),
      });
      setShowImport(false);
      setImportUrl("");
      await reload();
    } catch (e) {
      alert(`Import failed: ${e}`);
    }
  };

  const handleSetOpenApp = async (button: JoyConButton, bundleId: string) => {
    const payload = { kind: "open_app", bundle_id: bundleId };
    await invoke("plugin:joycon|joycon_set_mapping", {
      button,
      payload,
      mode: "tap",
    });
    setAppPickerFor(null);
    await reload();
  };

  const handleClearMapping = async (button: JoyConButton) => {
    await invoke("plugin:joycon|joycon_set_mapping", {
      button,
      payload: null,
      mode: "hold",
    });
    await reload();
  };

  const rowsByButton = new Map(mappings.map((m) => [m.button, m]));
  const rows = ALL_BUTTONS.map(
    (b) =>
      rowsByButton.get(b) ?? {
        button: b,
        payload: null,
        mode: "hold" as TriggerMode,
      },
  );

  const connected = resolveConnectedControllers(status, controllers);
  const connectedKinds = status.connected ? connected.map((c) => c.kind) : [];

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold">Joy-Con Controller</h1>
          <p className="text-sm text-text-secondary">
            {status.connected
              ? `● ${
                  connected.length > 0
                    ? connected
                        .map((c) => CONTROLLER_NAMES[c.kind] ?? "Controller")
                        .join(" + ")
                    : `${status.device_count} Joy-Con`
                } · ${status.battery}%${status.charging ? " ⚡" : ""}`
              : "○ Not connected — pair via macOS Bluetooth settings"}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => handleSetEnabled(e.target.checked)}
            />
            Enable
          </label>
          <button
            className="text-sm px-3 py-1.5 rounded-mac bg-accent text-white hover:bg-accent-hover"
            onClick={onLaunchWizard}
          >
            设置向导
          </button>
        </div>
      </div>

      <div className="surface-card p-4 flex justify-center">
        <Schematic
          layout={layout}
          highlight={highlight}
          active={activeBtns}
          labels={labels}
          connectedKinds={connectedKinds}
          className="w-full max-w-md"
        />
      </div>

      <div className="surface-card p-3">
        <div className="flex items-center gap-2 flex-wrap">
          <span className="text-sm font-medium">Preset:</span>
          {presets.map((p) => (
            <button
              key={p.id}
              className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent hover:text-accent"
              onClick={() => handleLoadPreset(p.id)}
              title={p.description}
            >
              {p.name}
            </button>
          ))}
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent hover:text-accent ml-auto"
            onClick={() => setShowImport((v) => !v)}
          >
            Import URL…
          </button>
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent hover:text-accent"
            onClick={handleExport}
          >
            Export
          </button>
        </div>
        {showImport && (
          <div className="mt-2 flex gap-2">
            <input
              type="text"
              placeholder="https://example.com/preset.json"
              className="flex-1 px-2 py-1 text-sm bg-surface-2 border border-border rounded"
              value={importUrl}
              onChange={(e) => setImportUrl(e.target.value)}
            />
            <button
              className="text-xs px-2 py-1 rounded-md bg-accent text-white"
              onClick={handleImportUrl}
            >
              Load
            </button>
          </div>
        )}
      </div>

      <div className="surface-card overflow-hidden">
        <div className="px-4 py-2 border-b border-border flex items-center justify-between">
          <h3 className="text-sm font-semibold">Mappings</h3>
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent"
            onClick={handleReset}
          >
            Reset
          </button>
        </div>        <table className="w-full text-sm">
          <thead className="bg-surface-2">
            <tr>
              <th className="text-left px-3 py-2 font-medium w-24">Button</th>
              <th className="text-left px-3 py-2 font-medium">Action</th>
              <th className="text-left px-3 py-2 font-medium w-32">Mode</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const builtinId =
                row.payload?.kind === "builtin" ? row.payload.id : "";
              return (
                <tr key={row.button} className="border-t border-border">
                  <td className="px-3 py-2 font-mono text-xs">
                    {BUTTON_LABELS[row.button]}
                  </td>
                  <td className="px-3 py-2">
                    <select
                      className="bg-surface-2 border border-border rounded px-2 py-1 text-xs"
                      value={builtinId}
                      onChange={(e) =>
                        handleSetAction(
                          row.button,
                          e.target.value || null,
                          row.mode,
                        )
                      }
                      disabled={!enabled}
                    >
                      <option value="">— unmapped —</option>
                      {actions.map((a) => (
                        <option key={a} value={a}>
                          {a}
                        </option>
                      ))}
                    </select>
                    {row.payload?.kind === "keyboard" && (
                      <span className="text-xs text-accent ml-2">[macro]</span>
                    )}
                    {row.payload?.kind === "text" && (
                      <span className="text-xs text-accent ml-2">[text]</span>
                    )}
                    {row.payload?.kind === "open_app" && (
                      <span className="text-xs text-accent ml-2">
                        → {row.payload.bundle_id}
                      </span>
                    )}
                    <button
                      className="text-xs ml-2 px-1.5 py-0.5 rounded border border-border hover:border-accent"
                      onClick={() => setAppPickerFor(row.button)}
                      disabled={!enabled}
                    >
                      App…
                    </button>
                    {row.payload && (
                      <button
                        className="text-xs ml-1 px-1.5 py-0.5 rounded border border-border hover:border-danger hover:text-danger"
                        onClick={() => handleClearMapping(row.button)}
                      >
                        ✕
                      </button>
                    )}
                  </td>
                  <td className="px-3 py-2">
                    <select
                      className="bg-surface-2 border border-border rounded px-2 py-1 text-xs"
                      value={row.mode}
                      onChange={(e) =>
                        handleSetAction(
                          row.button,
                          builtinId || null,
                          e.target.value as TriggerMode,
                        )
                      }
                      disabled={!enabled || !row.payload}
                    >
                      {(Object.keys(MODE_LABELS) as TriggerMode[]).map((m) => (
                        <option key={m} value={m}>
                          {MODE_LABELS[m]}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {appPickerFor && (
        <div
          className="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
          onClick={() => setAppPickerFor(null)}
        >
          <div
            className="surface-card max-w-md w-full max-h-[70vh] flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-4 py-3 border-b border-border flex items-center justify-between">
              <h3 className="text-sm font-semibold">
                Choose app for {BUTTON_LABELS[appPickerFor]}
              </h3>
              <button
                className="text-xs text-text-secondary hover:text-text"
                onClick={() => setAppPickerFor(null)}
              >
                ✕
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              {apps.length === 0 ? (
                <div className="p-4 text-xs text-text-secondary text-center">
                  No apps found in /Applications
                </div>
              ) : (
                apps.map((a) => (
                  <button
                    key={a.path}
                    className="w-full text-left px-4 py-2 hover:bg-accent/10 border-b border-border"
                    onClick={() =>
                      handleSetOpenApp(
                        appPickerFor,
                        a.bundle_id || a.name,
                      )
                    }
                  >
                    <div className="text-sm font-medium">{a.name}</div>
                    <div className="text-xs text-text-secondary">
                      {a.bundle_id || a.path}
                    </div>
                  </button>
                ))
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
