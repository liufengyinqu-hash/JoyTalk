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

function resolveSchematicLayout(
  connected: { kind: ControllerKind; serial: string }[],
): SchematicLayout {
  if (
    connected.some((c) => c.kind === "pro_controller") &&
    !connected.some(
      (c) => c.kind === "joy_con_left" || c.kind === "joy_con_right",
    )
  ) {
    return "pro";
  }
  return "pair";
}

function formatConnectionStatus(
  connected: { kind: ControllerKind; serial: string }[],
): string {
  if (connected.length === 0) {
    return "○ 未连接 — 请在 macOS 蓝牙设置中配对";
  }
  const left = connected.some((c) => c.kind === "joy_con_left");
  const right = connected.some((c) => c.kind === "joy_con_right");
  const pro = connected.some((c) => c.kind === "pro_controller");
  const parts: string[] = [];
  if (left || right || connected.length === 0) {
    parts.push(`L ${left ? "●" : "○"}`);
    parts.push(`R ${right ? "●" : "○"}`);
  }
  if (pro) {
    parts.push("Pro ●");
  }
  return `● ${parts.join(" · ")}`;
}

const MODE_ORDER: TriggerMode[] = [
  "tap",
  "double_tap",
  "hold",
  "long_press",
  "repeat",
];

const MODE_SHORT: Record<TriggerMode, string> = {
  hold: "按",
  tap: "单",
  double_tap: "双",
  long_press: "长",
  repeat: "连",
};

function buildDisplayRows(
  mappings: ButtonMapping[],
  drafts: ButtonMapping[],
): ButtonMapping[] {
  const rows: ButtonMapping[] = [...mappings, ...drafts];
  for (const b of ALL_BUTTONS) {
    if (!rows.some((r) => r.button === b)) {
      rows.push({ button: b, payload: null, mode: "hold" });
    }
  }
  return rows.sort((a, b) => {
    const bi = ALL_BUTTONS.indexOf(a.button) - ALL_BUTTONS.indexOf(b.button);
    if (bi !== 0) return bi;
    return MODE_ORDER.indexOf(a.mode) - MODE_ORDER.indexOf(b.mode);
  });
}

function rowKey(row: ButtonMapping): string {
  return `${row.button}:${row.mode}`;
}

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
  const [controllers, setControllers] = useState<
    { kind: ControllerKind; serial: string }[]
  >([]);
  const [presets, setPresets] = useState<{ id: string; name: string; description: string }[]>([]);
  const [showImport, setShowImport] = useState(false);
  const [importUrl, setImportUrl] = useState("");
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [appPickerFor, setAppPickerFor] = useState<{
    button: JoyConButton;
    mode: TriggerMode;
  } | null>(null);
  const [draftRows, setDraftRows] = useState<ButtonMapping[]>([]);

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
      setControllers(known);
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
        setControllers(known);
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
      const tagged = `${MODE_SHORT[m.mode]}:${name}`;
      acc[m.button] = acc[m.button] ? `${acc[m.button]} / ${tagged}` : tagged;
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
    if (payload) {
      await invoke("plugin:joycon|joycon_set_mapping", { button, payload, mode });
      setDraftRows((prev) =>
        prev.filter((d) => !(d.button === button && d.mode === mode)),
      );
    }
    await reload();
  };

  const handleModeChange = async (
    row: ButtonMapping,
    newMode: TriggerMode,
  ) => {
    if (row.mode === newMode) return;
    if (!row.payload) {
      setDraftRows((prev) => {
        const withoutButton = prev.filter((d) => d.button !== row.button);
        return [
          ...withoutButton,
          { button: row.button, payload: null, mode: newMode },
        ];
      });
      return;
    }
    await invoke("plugin:joycon|joycon_set_mapping", {
      button: row.button,
      payload: null,
      mode: row.mode,
    });
    await invoke("plugin:joycon|joycon_set_mapping", {
      button: row.button,
      payload: row.payload,
      mode: newMode,
    });
    await reload();
  };

  const handleAddMode = (button: JoyConButton) => {
    const used = new Set<TriggerMode>([
      ...mappings.filter((m) => m.button === button).map((m) => m.mode),
      ...draftRows.filter((d) => d.button === button).map((d) => d.mode),
    ]);
    const next = MODE_ORDER.find((m) => !used.has(m));
    if (!next) return;
    setDraftRows((prev) => [...prev, { button, payload: null, mode: next }]);
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

  const handleSetOpenApp = async (
    button: JoyConButton,
    mode: TriggerMode,
    bundleId: string,
  ) => {
    const payload = { kind: "open_app", bundle_id: bundleId };
    await invoke("plugin:joycon|joycon_set_mapping", {
      button,
      payload,
      mode,
    });
    setAppPickerFor(null);
    await reload();
  };

  const handleClearMapping = async (button: JoyConButton, mode: TriggerMode) => {
    setDraftRows((prev) =>
      prev.filter((d) => !(d.button === button && d.mode === mode)),
    );
    await invoke("plugin:joycon|joycon_set_mapping", {
      button,
      payload: null,
      mode,
    });
    await reload();
  };

  const rows = buildDisplayRows(mappings, draftRows);
  const firstRowForButton = new Set<JoyConButton>();

  const connected = resolveConnectedControllers(status, controllers);
  const connectedKinds = connected.map((c) => c.kind);
  const schematicLayout = resolveSchematicLayout(connected);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold">Joy-Con Controller</h1>
          <p className="text-sm text-text-secondary">
            {formatConnectionStatus(connected)}
            {status.connected && status.battery > 0
              ? ` · ${status.battery}%${status.charging ? " ⚡" : ""}`
              : ""}
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
          layout={schematicLayout}
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
        </div>
        <table className="w-full text-sm">
          <thead className="bg-surface-2">
            <tr>
              <th className="text-left px-3 py-2 font-medium w-28">Button</th>
              <th className="text-left px-3 py-2 font-medium">Action</th>
              <th className="text-left px-3 py-2 font-medium w-32">Mode</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const builtinId =
                row.payload?.kind === "builtin" ? row.payload.id : "";
              const showButtonLabel = !firstRowForButton.has(row.button);
              if (showButtonLabel) {
                firstRowForButton.add(row.button);
              }
              const usedModes = new Set(
                rows
                  .filter((r) => r.button === row.button && r.mode !== row.mode)
                  .map((r) => r.mode),
              );
              const modesForButton = new Set([
                ...mappings
                  .filter((m) => m.button === row.button)
                  .map((m) => m.mode),
                ...draftRows
                  .filter((d) => d.button === row.button)
                  .map((d) => d.mode),
              ]);
              if (!row.payload) {
                modesForButton.add(row.mode);
              }
              const allModesUsed = MODE_ORDER.every((m) =>
                modesForButton.has(m),
              );

              return (
                <tr key={rowKey(row)} className="border-t border-border">
                  <td className="px-3 py-2 font-mono text-xs">
                    {showButtonLabel ? (
                      <div className="flex items-center gap-1">
                        <span>{BUTTON_LABELS[row.button]}</span>
                        {!allModesUsed && (
                          <button
                            type="button"
                            className="text-[10px] px-1 rounded border border-border hover:border-accent text-text-secondary hover:text-accent"
                            title="添加触发方式"
                            onClick={() => handleAddMode(row.button)}
                            disabled={!enabled}
                          >
                            +
                          </button>
                        )}
                      </div>
                    ) : (
                      <span className="text-text-secondary">↳</span>
                    )}
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
                      onClick={() =>
                        setAppPickerFor({ button: row.button, mode: row.mode })
                      }
                      disabled={!enabled}
                    >
                      App…
                    </button>
                    {row.payload && (
                      <button
                        className="text-xs ml-1 px-1.5 py-0.5 rounded border border-border hover:border-danger hover:text-danger"
                        onClick={() =>
                          handleClearMapping(row.button, row.mode)
                        }
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
                        handleModeChange(row, e.target.value as TriggerMode)
                      }
                      disabled={!enabled}
                    >
                      {(Object.keys(MODE_LABELS) as TriggerMode[]).map((m) => (
                        <option
                          key={m}
                          value={m}
                          disabled={usedModes.has(m)}
                        >
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
                Choose app for {BUTTON_LABELS[appPickerFor.button]} (
                {MODE_LABELS[appPickerFor.mode]})
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
                        appPickerFor.button,
                        appPickerFor.mode,
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
