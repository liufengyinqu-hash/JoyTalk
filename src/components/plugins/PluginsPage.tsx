import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { BUILTIN_PLUGINS, PluginManifest } from "./manifest";
import { PluginCard } from "./PluginCard";
import { JoyConDetail } from "./joycon/JoyConDetail";
import { JoyConWizard } from "./joycon/Wizard";
import { MacroRecorder } from "./joycon/MacroRecorder";
import { ScriptRecorder } from "./joycon/ScriptRecorder";
import { ImuTuner } from "./joycon/ImuTuner";
import { ProfilesManager } from "./joycon/ProfilesManager";

type View =
  | { kind: "list"; tab: "discover" | "installed" }
  | { kind: "detail"; pluginId: string }
  | { kind: "wizard"; pluginId: string }
  | { kind: "macro" }
  | { kind: "script" }
  | { kind: "imu" }
  | { kind: "profiles" };

export const PluginsPage: React.FC = () => {
  const { t } = useTranslation();
  const [view, setView] = useState<View>({ kind: "list", tab: "discover" });

  const goDetail = (id: string) => setView({ kind: "detail", pluginId: id });
  const goList = () => setView({ kind: "list", tab: "discover" });

  if (view.kind === "wizard" && view.pluginId === "joycon") {
    return (
      <JoyConWizard
        onDone={() => setView({ kind: "detail", pluginId: "joycon" })}
        onCancel={() => setView({ kind: "detail", pluginId: "joycon" })}
      />
    );
  }

  if (view.kind === "macro") {
    return <MacroRecorder onClose={() => setView({ kind: "detail", pluginId: "joycon" })} />;
  }

  if (view.kind === "script") {
    return <ScriptRecorder onClose={() => setView({ kind: "detail", pluginId: "joycon" })} />;
  }

  if (view.kind === "imu") {
    return <ImuTuner onClose={() => setView({ kind: "detail", pluginId: "joycon" })} />;
  }

  if (view.kind === "profiles") {
    return <ProfilesManager onClose={() => setView({ kind: "detail", pluginId: "joycon" })} />;
  }

  if (view.kind === "detail" && view.pluginId === "joycon") {
    return (
      <div className="max-w-4xl w-full mx-auto space-y-3">
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={goList}
        >
          ← Plugins
        </button>
        <JoyConDetail
          onLaunchWizard={() =>
            setView({ kind: "wizard", pluginId: "joycon" })
          }
        />
        <div className="flex justify-end gap-2 flex-wrap">
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent"
            onClick={() => setView({ kind: "macro" })}
          >
            + Macro
          </button>
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent"
            onClick={() => setView({ kind: "script" })}
          >
            + Script
          </button>
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent"
            onClick={() => setView({ kind: "imu" })}
          >
            IMU Tuning
          </button>
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent"
            onClick={() => setView({ kind: "profiles" })}
          >
            Per-App Profiles
          </button>
        </div>
      </div>
    );
  }

  // List view
  if (view.kind !== "list") return null;
  const tabs = [
    { id: "discover" as const, label: t("plugins.tab.discover", "Discover") },
    { id: "installed" as const, label: t("plugins.tab.installed", "Installed") },
  ];
  const currentTab = view.tab;
  const items: PluginManifest[] =
    currentTab === "installed"
      ? BUILTIN_PLUGINS.filter((p) => p.builtin)
      : BUILTIN_PLUGINS;

  return (
    <div className="max-w-4xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold">{t("plugins.title", "Plugins")}</h1>
          <p className="text-sm text-text-secondary">
            {t(
              "plugins.subtitle",
              "Extend JoyTalk with input devices, audio sources, and post-processing",
            )}
          </p>
        </div>
        <div className="flex gap-1 p-1 surface-card">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setView({ kind: "list", tab: tab.id })}
              className={`text-xs px-3 py-1 rounded ${
                currentTab === tab.id
                  ? "bg-accent text-white"
                  : "text-text-secondary hover:text-text"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {items.map((plugin) => (
          <PluginCard
            key={plugin.id}
            plugin={plugin}
            installed={plugin.builtin}
            onOpen={() => goDetail(plugin.id)}
          />
        ))}
      </div>
    </div>
  );
};
