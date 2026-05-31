import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { useSettings } from "../../../hooks/useSettings";
import { BindingEditor } from "./BindingEditor";
import {
  ALL_BUTTONS,
  BUTTON_LABELS,
  ButtonMapping,
  JoyConButton,
  JoyConStatus,
} from "../../plugins/joycon/types";

interface BindingProps {
  actionId: "transcribe" | "cancel";
  label: string;
  description: string;
  mappings: ButtonMapping[];
  onChange: (button: JoyConButton | null) => void;
  capturing: boolean;
  onStartCapture: () => void;
  onStopCapture: () => void;
}

const BindingRow: React.FC<BindingProps> = ({
  actionId,
  label,
  description,
  mappings,
  onChange,
  capturing,
  onStartCapture,
  onStopCapture,
}) => {
  const { t } = useTranslation();
  const current = mappings.find(
    (m) =>
      m.payload?.kind === "builtin" &&
      m.payload.id === actionId,
  )?.button ?? null;

  return (
    <SettingContainer
      title={label}
      description={description}
      descriptionMode="tooltip"
      grouped
    >
      <div className="flex items-center gap-2">
        <select
          className="bg-surface-2 border border-border rounded px-2 py-1 text-xs min-w-[120px]"
          value={current ?? ""}
          onChange={(e) =>
            onChange((e.target.value as JoyConButton) || null)
          }
        >
          <option value="">{t("joycon.bindings.none", "— none —")}</option>
          {ALL_BUTTONS.map((b) => (
            <option key={b} value={b}>
              {BUTTON_LABELS[b]}
            </option>
          ))}
        </select>
        {capturing ? (
          <button
            className="text-xs px-2 py-1 rounded border border-danger text-danger"
            onClick={onStopCapture}
          >
            {t("common.cancel", "Cancel")}
          </button>
        ) : (
          <button
            className="text-xs px-2 py-1 rounded border border-border hover:border-accent"
            onClick={onStartCapture}
            title={t(
              "joycon.bindings.capture.hint",
              "Press a Joy-Con button to capture",
            )}
          >
            {t("joycon.bindings.capture.btn", "Press JC")}
          </button>
        )}
      </div>
    </SettingContainer>
  );
};

export const JoyConBindings: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const pttEnabled = (getSetting("push_to_talk") as boolean) ?? false;

  const [enabled, setEnabled] = useState(true);
  const [mappings, setMappings] = useState<ButtonMapping[]>([]);
  const [status, setStatus] = useState<JoyConStatus>({
    connected: false,
    battery: 0,
    charging: false,
    device_count: 0,
  });
  const [captureFor, setCaptureFor] = useState<
    "transcribe" | "cancel" | null
  >(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorInitial, setEditorInitial] = useState<ButtonMapping | null>(
    null,
  );
  const reload = async () => {
    try {
      const [en, mp, st] = await Promise.all([
        invoke<boolean>("plugin:joycon|joycon_get_enabled"),
        invoke<ButtonMapping[]>("plugin:joycon|joycon_get_mappings"),
        invoke<JoyConStatus>("plugin:joycon|joycon_get_status"),
      ]);
      setEnabled(en);
      setMappings(mp);
      setStatus(st);
    } catch (e) {
      console.warn("[joycon-bindings] reload failed", e);
    }
  };

  useEffect(() => {
    let alive = true;
    const unlisteners: Array<() => void> = [];
    (async () => {
      const u1 = await listen<JoyConStatus>("joycon://status", (e) => {
        if (alive) setStatus(e.payload);
      });
      unlisteners.push(u1);
      const u2 = await listen<{
        button: JoyConButton;
        pressed: boolean;
      }>("joycon://button", async (e) => {
        if (!alive || !captureFor || !e.payload.pressed) return;
        const action = captureFor;
        await assignBinding(action, e.payload.button);
        await invoke("plugin:joycon|joycon_stop_capture").catch(() => {});
        setCaptureFor(null);
      });
      unlisteners.push(u2);
      await reload();
    })();
    return () => {
      alive = false;
      unlisteners.forEach((u) => u());
    };
  }, [captureFor]);

  const assignBinding = async (
    action: "transcribe" | "cancel",
    button: JoyConButton | null,
  ) => {
    // Clear the existing button currently bound to THIS action only.
    // We deliberately do NOT clear other actions on the same button when
    // push-to-talk is off — user is allowed to map transcribe and cancel
    // to the same button (release = stop = effectively cancel in PTT).
    const existing = mappings.find(
      (m) =>
        m.payload?.kind === "builtin" && m.payload.id === action,
    );
    if (existing && (!button || existing.button !== button)) {
      // existing slot held only this action → safe to null it
      const stillUsedByOther = mappings.find(
        (m) =>
          m.button === existing.button &&
          m.payload?.kind === "builtin" &&
          m.payload.id !== action,
      );
      if (!stillUsedByOther) {
        await invoke("plugin:joycon|joycon_set_mapping", {
          button: existing.button,
          payload: null,
          mode: existing.mode,
        });
      }
    }
    if (button) {
      const mode = action === "transcribe" && pttEnabled ? "hold" : "tap";
      await invoke("plugin:joycon|joycon_set_mapping", {
        button,
        payload: { kind: "builtin", id: action },
        mode,
      });
    }
    await reload();
  };

  const handleSetEnabled = async (v: boolean) => {
    await invoke("plugin:joycon|joycon_set_enabled", { enabled: v });
    setEnabled(v);
  };

  const startCapture = async (action: "transcribe" | "cancel") => {
    setCaptureFor(action);
    await invoke("plugin:joycon|joycon_start_capture").catch(() => {});
  };

  const stopCapture = async () => {
    setCaptureFor(null);
    await invoke("plugin:joycon|joycon_stop_capture").catch(() => {});
  };

  // re-apply mode when push-to-talk toggles, so transcribe binding
  // matches keyboard behavior. Also: when PTT turns on, hide-implies-clear
  // any cancel mapping (release of transcribe key acts as stop).
  useEffect(() => {
    const m = mappings.find(
      (m) => m.payload?.kind === "builtin" && m.payload.id === "transcribe",
    );
    if (m) {
      const desired = pttEnabled ? "hold" : "tap";
      if (m.mode !== desired) {
        invoke("plugin:joycon|joycon_set_mapping", {
          button: m.button,
          payload: null,
          mode: m.mode,
        })
          .then(() =>
            invoke("plugin:joycon|joycon_set_mapping", {
              button: m.button,
              payload: m.payload,
              mode: desired,
            }),
          )
          .then(reload);
      }
    }
    if (pttEnabled) {
      const cancelMap = mappings.find(
        (m) => m.payload?.kind === "builtin" && m.payload.id === "cancel",
      );
      if (cancelMap) {
        // Don't wipe if same button also holds transcribe (shared key)
        const alsoTranscribe = mappings.find(
          (mm) =>
            mm.button === cancelMap.button &&
            mm.payload?.kind === "builtin" &&
            mm.payload.id === "transcribe",
        );
        if (!alsoTranscribe) {
          invoke("plugin:joycon|joycon_set_mapping", {
            button: cancelMap.button,
            payload: null,
            mode: cancelMap.mode,
          }).then(reload);
        }
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pttEnabled]);

  const statusText = status.connected
    ? `● ${status.device_count} Joy-Con · ${status.battery}%${status.charging ? " ⚡" : ""}`
    : t(
        "joycon.bindings.disconnected",
        "○ Not connected — pair via macOS Bluetooth settings",
      );

  return (
    <SettingsGroup title={t("joycon.bindings.title", "Joy-Con Bindings")}>
      <ToggleSwitch
        checked={enabled}
        onChange={handleSetEnabled}
        label={t("joycon.bindings.enable.label", "Enable Joy-Con")}
        description={t(
          "joycon.bindings.enable.desc",
          "Use a Nintendo Switch Joy-Con as input source",
        )}
        descriptionMode="tooltip"
        grouped
      />
      <BindingRow
        actionId="transcribe"
        label={t("joycon.bindings.transcribe.label", "Transcribe button")}
        description={t(
          "joycon.bindings.transcribe.desc",
          "Joy-Con button to start (or hold for) transcription",
        )}
        mappings={mappings}
        capturing={captureFor === "transcribe"}
        onStartCapture={() => startCapture("transcribe")}
        onStopCapture={stopCapture}
        onChange={(b) => assignBinding("transcribe", b)}
      />
      {!pttEnabled && (
        <BindingRow
          actionId="cancel"
          label={t("joycon.bindings.cancel.label", "Cancel button")}
          description={t(
            "joycon.bindings.cancel.desc",
            "Joy-Con button to cancel an active recording",
          )}
          mappings={mappings}
          capturing={captureFor === "cancel"}
          onStartCapture={() => startCapture("cancel")}
          onStopCapture={stopCapture}
          onChange={(b) => assignBinding("cancel", b)}
        />
      )}
      <ToggleSwitch
        checked={pttEnabled}
        onChange={(v) => updateSetting("push_to_talk", v)}
        isUpdating={isUpdating("push_to_talk")}
        label={t("joycon.bindings.ptt.label", "Push-to-talk")}
        description={t(
          "joycon.bindings.ptt.desc",
          "Hold the transcribe button to record (release to stop). Off = tap to toggle.",
        )}
        descriptionMode="tooltip"
        grouped
      />
      <SettingContainer
        title={t("joycon.bindings.status.label", "Status")}
        description={t("joycon.bindings.status.desc", "Joy-Con connection state")}
        descriptionMode="tooltip"
        grouped
      >
        <span className="text-xs">{statusText}</span>
      </SettingContainer>

      {/* Custom mappings (keyboard macros, shell, AppleScript, etc.) */}
      <CustomMappingsList
        mappings={mappings}
        onEdit={(m) => {
          setEditorInitial(m);
          setEditorOpen(true);
        }}
        onDelete={async (m) => {
          await invoke("plugin:joycon|joycon_set_mapping", {
            button: m.button,
            payload: null,
            mode: m.mode,
          });
          await reload();
        }}
        onAdd={() => {
          setEditorInitial(null);
          setEditorOpen(true);
        }}
      />

      <BindingEditor
        open={editorOpen}
        initial={editorInitial}
        onClose={() => setEditorOpen(false)}
        onSaved={reload}
      />
    </SettingsGroup>
  );
};

interface CustomListProps {
  mappings: ButtonMapping[];
  onEdit: (m: ButtonMapping) => void;
  onDelete: (m: ButtonMapping) => void;
  onAdd: () => void;
}

const CustomMappingsList: React.FC<CustomListProps> = ({
  mappings,
  onEdit,
  onDelete,
  onAdd,
}) => {
  // Filter: only non-builtin-transcribe/cancel mappings (shown elsewhere) and assigned ones
  const custom = mappings.filter((m) => {
    if (!m.payload) return false;
    if (
      m.payload.kind === "builtin" &&
      (m.payload.id === "transcribe" || m.payload.id === "cancel")
    ) {
      return false;
    }
    return true;
  });

  const summary = (m: ButtonMapping): string => {
    const p = m.payload;
    if (!p) return "—";
    switch (p.kind) {
      case "builtin":
        return `内置: ${p.id}`;
      case "keyboard":
        return `键盘宏 (${p.chords.length} 步)`;
      case "text":
        return `文本: ${p.text.slice(0, 32)}${p.text.length > 32 ? "…" : ""}`;
      case "open_app":
        return `打开: ${p.bundle_id}`;
      case "shell":
        return `Shell: ${p.command.slice(0, 32)}${p.command.length > 32 ? "…" : ""}`;
      case "apple_script":
        return `AS: ${p.script.slice(0, 32)}${p.script.length > 32 ? "…" : ""}`;
    }
  };

  return (
    <div className="px-3.5 py-2.5">
      <div className="flex items-center justify-between mb-2">
        <div>
          <h3 className="text-sm font-medium">自定义映射</h3>
          <p className="text-xs text-text-secondary">
            键盘宏 / 文本 / 打开 App / Shell / AppleScript
          </p>
        </div>
        <button
          className="text-xs px-2 py-1 rounded-md bg-accent text-white hover:bg-accent-hover"
          onClick={onAdd}
        >
          + 新增
        </button>
      </div>
      {custom.length === 0 ? (
        <p className="text-xs text-text-secondary py-3 text-center surface-2 rounded-mac">
          暂无自定义映射. 点 "+ 新增" 给任意按键配置键盘快捷键 / 宏 / 脚本.
        </p>
      ) : (
        <ul className="space-y-1.5">
          {custom.map((m) => (
            <li
              key={m.button}
              className="flex items-center gap-2 surface-2 rounded-mac px-3 py-2 text-sm"
            >
              <span className="font-mono text-xs px-2 py-0.5 rounded bg-accent/15 text-accent w-14 text-center">
                {BUTTON_LABELS[m.button]}
              </span>
              <span className="flex-1 truncate text-xs">{summary(m)}</span>
              <span className="text-[10px] text-text-secondary uppercase">
                {m.mode}
              </span>
              <button
                className="text-xs px-2 py-0.5 rounded border border-border hover:border-accent"
                onClick={() => onEdit(m)}
              >
                编辑
              </button>
              <button
                className="text-xs px-2 py-0.5 rounded border border-border hover:border-danger hover:text-danger"
                onClick={() => onDelete(m)}
              >
                删除
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};
