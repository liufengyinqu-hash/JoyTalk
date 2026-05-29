import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ButtonMapping } from "./types";

interface AppEntry {
  name: string;
  bundle_id: string;
  path: string;
}

interface AppProfile {
  bundle_id: string;
  mappings: ButtonMapping[];
}

interface Props {
  onClose: () => void;
}

export const ProfilesManager: React.FC<Props> = ({ onClose }) => {
  const [enabled, setEnabled] = useState(false);
  const [profiles, setProfiles] = useState<AppProfile[]>([]);
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [frontmost, setFrontmost] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);

  const reload = async () => {
    const [en, ps, a, fm] = await Promise.all([
      invoke<boolean>("plugin:joycon|joycon_get_per_app_enabled"),
      invoke<AppProfile[]>("plugin:joycon|joycon_get_profiles"),
      invoke<AppEntry[]>("plugin:joycon|joycon_list_apps").catch(() => []),
      invoke<string | null>("plugin:joycon|joycon_get_frontmost").catch(() => null),
    ]);
    setEnabled(en);
    setProfiles(ps);
    setApps(a);
    setFrontmost(fm);
  };

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    (async () => {
      await reload();
      const u = await listen<string | null>("joycon://frontmost_changed", (e) => {
        if (alive) setFrontmost(e.payload ?? null);
      });
      unlisten = u;
    })();
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const toggleEnabled = async (v: boolean) => {
    await invoke("plugin:joycon|joycon_set_per_app_enabled", { enabled: v });
    setEnabled(v);
  };

  const captureCurrent = async (bundleId: string) => {
    const mappings = await invoke<ButtonMapping[]>(
      "plugin:joycon|joycon_get_mappings",
    );
    await invoke("plugin:joycon|joycon_save_profile", {
      bundleId,
      mappings,
    });
    setPicking(false);
    await reload();
  };

  const delProfile = async (bundleId: string) => {
    await invoke("plugin:joycon|joycon_delete_profile", { bundleId });
    await reload();
  };

  return (
    <div className="max-w-2xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">Per-App Profiles</h1>
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={onClose}
        >
          ← Back
        </button>
      </div>

      <div className="surface-card p-4 space-y-3">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(e) => toggleEnabled(e.target.checked)}
          />
          Enable per-app profiles (auto-switch mappings by frontmost app)
        </label>
        <p className="text-xs text-text-secondary">
          Frontmost: <span className="font-mono">{frontmost ?? "—"}</span>
        </p>
      </div>

      <div className="surface-card overflow-hidden">
        <div className="px-4 py-2 border-b border-border flex items-center justify-between">
          <h3 className="text-sm font-semibold">Profiles ({profiles.length})</h3>
          <button
            className="text-xs px-2 py-1 rounded-md border border-border hover:border-accent"
            onClick={() => setPicking((v) => !v)}
          >
            + New from current
          </button>
        </div>
        {profiles.length === 0 ? (
          <div className="p-4 text-xs text-text-secondary text-center">
            No profiles yet. Click "+ New from current" to capture current
            mappings for an app.
          </div>
        ) : (
          <ul>
            {profiles.map((p) => {
              const app = apps.find((a) => a.bundle_id === p.bundle_id);
              const isFront = p.bundle_id === frontmost;
              return (
                <li
                  key={p.bundle_id}
                  className="px-4 py-2 border-t border-border flex items-center justify-between"
                >
                  <div>
                    <div className="text-sm font-medium flex items-center gap-2">
                      {app?.name ?? p.bundle_id}
                      {isFront && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-accent/15 text-accent">
                          ACTIVE
                        </span>
                      )}
                    </div>
                    <div className="text-xs text-text-secondary font-mono">
                      {p.bundle_id} · {p.mappings.length} mappings
                    </div>
                  </div>
                  <button
                    className="text-xs px-2 py-1 rounded border border-border hover:border-danger hover:text-danger"
                    onClick={() => delProfile(p.bundle_id)}
                  >
                    Delete
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {picking && (
        <div
          className="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
          onClick={() => setPicking(false)}
        >
          <div
            className="surface-card max-w-md w-full max-h-[70vh] flex flex-col overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="px-4 py-3 border-b border-border flex items-center justify-between">
              <h3 className="text-sm font-semibold">
                Choose app for new profile
              </h3>
              <button
                className="text-xs text-text-secondary hover:text-text"
                onClick={() => setPicking(false)}
              >
                ✕
              </button>
            </div>
            <div className="flex-1 overflow-y-auto">
              {frontmost && (
                <button
                  className="w-full text-left px-4 py-2 hover:bg-accent/10 border-b border-border bg-accent/5"
                  onClick={() => captureCurrent(frontmost)}
                >
                  <div className="text-sm font-medium">
                    Frontmost app
                  </div>
                  <div className="text-xs text-text-secondary font-mono">
                    {frontmost}
                  </div>
                </button>
              )}
              {apps.map((a) => (
                <button
                  key={a.path}
                  className="w-full text-left px-4 py-2 hover:bg-accent/10 border-b border-border"
                  onClick={() =>
                    captureCurrent(a.bundle_id || a.name)
                  }
                >
                  <div className="text-sm font-medium">{a.name}</div>
                  <div className="text-xs text-text-secondary font-mono">
                    {a.bundle_id || a.path}
                  </div>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
