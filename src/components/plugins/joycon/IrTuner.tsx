import React, { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { ToggleSwitch } from "@/components/ui/ToggleSwitch";

type McuMode = "off" | "ir" | "nfc";
type McuSubMode = "ir" | "nfc";

interface McuConfig {
  mode: McuMode;
  white_pixel_threshold: number;
}

interface IrLiveSample {
  session_active: boolean;
  average_intensity: number;
  white_pixel_count: number;
  ambient_noise_count: number;
  proximity_active: boolean;
}

interface NfcLiveSample {
  session_active: boolean;
  tag_present: boolean;
  tag_detected: boolean;
  uid: string;
  uid_len: number;
  tag_type: number;
  nfc_state: number;
}

interface McuStatus {
  config: McuConfig;
  active_mode: McuMode;
  switching: boolean;
}

interface Props {
  onClose: () => void;
}

const DEFAULTS: McuConfig = {
  mode: "nfc",
  white_pixel_threshold: 50,
};

const EMPTY_IR: IrLiveSample = {
  session_active: false,
  average_intensity: 0,
  white_pixel_count: 0,
  ambient_noise_count: 0,
  proximity_active: false,
};

const EMPTY_NFC: NfcLiveSample = {
  session_active: false,
  tag_present: false,
  tag_detected: false,
  uid: "",
  uid_len: 0,
  tag_type: 0,
  nfc_state: 0,
};

const HISTORY_LEN = 120;

function normalizeNfcSample(sample: NfcLiveSample): NfcLiveSample {
  if (sample.tag_present && sample.uid) return sample;
  return {
    ...sample,
    tag_present: false,
    tag_detected: sample.tag_detected && !sample.tag_present,
    uid: "",
    uid_len: 0,
    tag_type: 0,
  };
}
const SUB_MODES: McuSubMode[] = ["ir", "nfc"];

function normalizeConfig(raw: McuConfig & { enabled?: boolean }): McuConfig {
  if (raw.mode) {
    return {
      mode: raw.mode,
      white_pixel_threshold: raw.white_pixel_threshold ?? 50,
    };
  }
  if (raw.enabled === false) {
    return { mode: "off", white_pixel_threshold: raw.white_pixel_threshold ?? 50 };
  }
  return {
    mode: "nfc",
    white_pixel_threshold: raw.white_pixel_threshold ?? 50,
  };
}

export const IrTuner: React.FC<Props> = ({ onClose }) => {
  const { t } = useTranslation();
  const [mcu, setMcu] = useState<McuConfig>(DEFAULTS);
  const [activeMode, setActiveMode] = useState<McuMode>("off");
  const [switching, setSwitching] = useState(false);
  const [irLive, setIrLive] = useState<IrLiveSample>(EMPTY_IR);
  const [nfcLive, setNfcLive] = useState<NfcLiveSample>(EMPTY_NFC);
  const [history, setHistory] = useState<number[]>([]);
  const [uidHistory, setUidHistory] = useState<string[]>([]);
  const [lastNfcAt, setLastNfcAt] = useState<number | null>(null);
  const [copiedUid, setCopiedUid] = useState(false);
  const lastSubModeRef = useRef<McuSubMode>("nfc");

  const mcuEnabled = mcu.mode !== "off";
  const desiredActive: McuMode = mcuEnabled ? mcu.mode : "off";

  const applyStatus = (status: McuStatus) => {
    const next = normalizeConfig(status.config as McuConfig & { enabled?: boolean });
    if (next.mode === "ir" || next.mode === "nfc") {
      lastSubModeRef.current = next.mode;
    }
    setMcu(next);
    setActiveMode(status.active_mode);
    setSwitching(status.switching);
  };

  useEffect(() => {
    invoke<McuStatus>("plugin:joycon|joycon_get_ir")
      .then(applyStatus)
      .catch(() => setMcu(DEFAULTS));
    invoke<IrLiveSample>("plugin:joycon|joycon_get_ir_sample")
      .then(setIrLive)
      .catch(() => setIrLive(EMPTY_IR));
    invoke<NfcLiveSample>("plugin:joycon|joycon_get_nfc_sample")
      .then((sample) => setNfcLive(normalizeNfcSample(sample)))
      .catch(() => setNfcLive(EMPTY_NFC));
  }, []);

  useEffect(() => {
    let cancelled = false;
    const unsubs: (() => void)[] = [];

    listen<McuStatus>("joycon://mcu_status", (event) => {
      if (cancelled) return;
      applyStatus(event.payload);
    }).then((fn) => {
      if (cancelled) fn();
      else unsubs.push(fn);
    });

    listen<IrLiveSample>("joycon://ir_sample", (event) => {
      if (cancelled) return;
      const sample = event.payload;
      setIrLive(sample);
      setHistory((prev) =>
        [...prev, sample.white_pixel_count].slice(-HISTORY_LEN),
      );
    }).then((fn) => {
      if (cancelled) fn();
      else unsubs.push(fn);
    });

    listen<NfcLiveSample>("joycon://nfc_sample", (event) => {
      if (cancelled) return;
      const sample = normalizeNfcSample(event.payload);
      setNfcLive(sample);
      setLastNfcAt(Date.now());
      if (sample.tag_present && sample.uid) {
        setUidHistory((prev) =>
          prev[0] === sample.uid ? prev : [sample.uid, ...prev].slice(0, 8),
        );
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unsubs.push(fn);
    });

    return () => {
      cancelled = true;
      unsubs.forEach((fn) => fn());
    };
  }, []);

  const persist = (next: McuConfig) => {
    if (next.mode === "ir" || next.mode === "nfc") {
      lastSubModeRef.current = next.mode;
    }
    setMcu(next);
    invoke("plugin:joycon|joycon_set_ir", { ir: next }).catch(() => {});
  };

  const setMcuEnabled = (enabled: boolean) => {
    if (enabled) {
      persist({ ...mcu, mode: lastSubModeRef.current });
    } else {
      if (mcu.mode === "ir" || mcu.mode === "nfc") {
        lastSubModeRef.current = mcu.mode;
      }
      persist({ ...mcu, mode: "off" });
    }
  };

  const setSubMode = (mode: McuSubMode) => {
    persist({ ...mcu, mode });
  };

  const reset = async () => {
    setMcu(DEFAULTS);
    setHistory([]);
    setUidHistory([]);
    setLastNfcAt(null);
    await invoke("plugin:joycon|joycon_set_ir", { ir: DEFAULTS }).catch(() => {});
  };

  const viewMode: McuMode = mcuEnabled ? mcu.mode : activeMode;
  const sessionReady =
    viewMode === "ir"
      ? irLive.session_active
      : viewMode === "nfc"
        ? nfcLive.session_active
        : false;
  const isStartingMcu = mcuEnabled && activeMode !== mcu.mode && !switching;

  const copyUid = async () => {
    if (!nfcLive.tag_present || !nfcLive.uid) return;
    try {
      await navigator.clipboard.writeText(nfcLive.uid);
      setCopiedUid(true);
      window.setTimeout(() => setCopiedUid(false), 1500);
    } catch {
      /* ignore */
    }
  };

  const restartNfcScan = () => {
    invoke("plugin:joycon|joycon_restart_nfc_scan").catch(() => {});
  };

  const displayUid = nfcLive.tag_present ? nfcLive.uid : "";

  const readyToScan =
    sessionReady && !nfcLive.tag_present && !nfcLive.tag_detected;

  const scanning = sessionReady && !nfcLive.tag_present && nfcLive.tag_detected;

  const nfcStateLabel = (state: number) => {
    if (state === 0x09) return t("joycon.irTuner.nfcLive.stateTagPresent");
    if (state === 0x01) return t("joycon.irTuner.nfcLive.statePolling");
    if (state === 0x00) return t("joycon.irTuner.nfcLive.stateIdle");
    if (scanning) return t("joycon.irTuner.nfcLive.stateScanning");
    return t("joycon.irTuner.nfcLive.stateUnknown", {
      code: `0x${state.toString(16).padStart(2, "0")}`,
    });
  };

  const nfcZoneLabel = () => {
    if (!sessionReady) return t("joycon.irTuner.nfcLive.waitingSession");
    if (nfcLive.tag_present) return t("joycon.irTuner.nfcLive.tagOn");
    if (readyToScan) return t("joycon.irTuner.nfcLive.readyToScan");
    if (scanning) return t("joycon.irTuner.nfcLive.scanning");
    return t("joycon.irTuner.nfcLive.resetting");
  };

  const thresholdPct = Math.min(
    100,
    (irLive.white_pixel_count / Math.max(mcu.white_pixel_threshold, 1)) * 100,
  );
  const maxHistory = Math.max(...history, mcu.white_pixel_threshold, 1);
  const modeSynced = desiredActive === activeMode && !switching;

  return (
    <div className="max-w-2xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">{t("joycon.irTuner.title")}</h1>
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={onClose}
        >
          {t("joycon.irTuner.back")}
        </button>
      </div>

      <p className="text-sm text-text-secondary">{t("joycon.irTuner.subtitle")}</p>

      <div className="surface-card p-4 space-y-4">
        <ToggleSwitch
          checked={mcuEnabled}
          onChange={setMcuEnabled}
          label={t("joycon.irTuner.enabled.label")}
          description={t("joycon.irTuner.enabled.hint")}
          descriptionMode="inline"
        />

        {mcuEnabled && (
          <div className="space-y-2">
            <p className="text-sm">{t("joycon.irTuner.mode.label")}</p>
            <div className="flex flex-wrap gap-2">
              {SUB_MODES.map((mode) => (
                <button
                  key={mode}
                  type="button"
                  className={`text-xs px-3 py-1.5 rounded-mac border transition-colors ${
                    mcu.mode === mode
                      ? "border-accent bg-accent/10 text-accent"
                      : "border-border text-text-secondary hover:text-text"
                  }`}
                  onClick={() => setSubMode(mode)}
                >
                  {t(`joycon.irTuner.mode.${mode}`)}
                </button>
              ))}
            </div>
            <p className="text-xs text-text-secondary">
              {t("joycon.irTuner.mode.hint")}
            </p>
          </div>
        )}

        <div className="rounded-mac border border-border p-3 space-y-2 text-xs">
          <div className="grid grid-cols-3 gap-2">
            <div>
              <p className="text-text-secondary">{t("joycon.irTuner.status.config")}</p>
              <p className="font-medium mt-0.5">
                {mcuEnabled ? t(`joycon.irTuner.mode.${mcu.mode}`) : t("joycon.irTuner.mode.off")}
              </p>
            </div>
            <div>
              <p className="text-text-secondary">{t("joycon.irTuner.status.runtime")}</p>
              <p className="font-medium mt-0.5">
                {switching
                  ? t("joycon.irTuner.mode.switching")
                  : t(`joycon.irTuner.mode.${activeMode}`)}
              </p>
            </div>
            <div>
              <p className="text-text-secondary">{t("joycon.irTuner.status.session")}</p>
              <p
                className={`font-medium mt-0.5 ${
                  isStartingMcu
                    ? "text-amber-600 dark:text-amber-400"
                    : sessionReady
                      ? "text-green-600 dark:text-green-400"
                      : "text-text-secondary"
                }`}
              >
                {isStartingMcu
                  ? t("joycon.irTuner.mode.pending")
                  : sessionReady
                    ? t("joycon.irTuner.live.sessionActive")
                    : t("joycon.irTuner.live.sessionInactive")}
              </p>
            </div>
          </div>
          {mcuEnabled && !modeSynced && !switching && !isStartingMcu && (
            <p className="text-text-secondary">{t("joycon.irTuner.mode.pending")}</p>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-2 text-xs">
          <span className="text-text-secondary">
            {t("joycon.irTuner.mode.activeLabel")}
          </span>
          <span
            className={`font-medium px-2 py-0.5 rounded-full ${
              switching || isStartingMcu
                ? "bg-amber-500/15 text-amber-600 dark:text-amber-400"
                : modeSynced && sessionReady
                  ? "bg-green-500/15 text-green-600 dark:text-green-400"
                  : "bg-text-secondary/10 text-text-secondary"
            }`}
          >
            {switching || isStartingMcu
              ? t("joycon.irTuner.mode.switching")
              : t(`joycon.irTuner.mode.${activeMode}`)}
          </span>
        </div>

        {mcu.mode === "ir" && (
          <div className="space-y-1">
            <div className="flex justify-between text-sm">
              <label>{t("joycon.irTuner.threshold.label")}</label>
              <span className="font-mono text-xs text-text-secondary">
                {mcu.white_pixel_threshold}
              </span>
            </div>
            <input
              type="range"
              value={mcu.white_pixel_threshold}
              min={10}
              max={300}
              step={5}
              onChange={(e) =>
                persist({
                  ...mcu,
                  white_pixel_threshold: parseInt(e.target.value, 10),
                })
              }
              className="w-full"
            />
            <p className="text-xs text-text-secondary">
              {t("joycon.irTuner.threshold.hint")}
            </p>
          </div>
        )}

        {mcu.mode !== "off" && (
          <div className="rounded-mac border border-border p-3 space-y-3">
            <div className="flex items-center justify-between text-sm">
              <span>
                {viewMode === "ir"
                  ? t("joycon.irTuner.live.title")
                  : t("joycon.irTuner.nfcLive.title")}
              </span>
              {viewMode === "nfc" && lastNfcAt !== null && (
                <span className="text-[11px] text-text-secondary font-mono">
                  {t("joycon.irTuner.nfcLive.lastUpdate")}{" "}
                  {new Date(lastNfcAt).toLocaleTimeString()}
                </span>
              )}
            </div>

            {viewMode === "ir" && (
              <>
                <div
                  className={`flex items-center gap-2 text-sm font-medium ${
                    irLive.proximity_active ? "text-accent" : "text-text-secondary"
                  }`}
                >
                  <span
                    className={`inline-block w-2.5 h-2.5 rounded-full ${
                      irLive.proximity_active
                        ? "bg-accent animate-pulse"
                        : "bg-border"
                    }`}
                  />
                  {irLive.proximity_active
                    ? t("joycon.irTuner.live.proximityOn")
                    : t("joycon.irTuner.live.proximityOff")}
                </div>

                <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.live.intensity")}
                    </dt>
                    <dd className="font-mono text-sm">{irLive.average_intensity}</dd>
                  </div>
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.live.whitePixels")}
                    </dt>
                    <dd className="font-mono text-sm">{irLive.white_pixel_count}</dd>
                  </div>
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.live.ambient")}
                    </dt>
                    <dd className="font-mono text-sm">{irLive.ambient_noise_count}</dd>
                  </div>
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.live.threshold")}
                    </dt>
                    <dd className="font-mono text-sm">{mcu.white_pixel_threshold}</dd>
                  </div>
                </dl>

                <div className="space-y-1">
                  <div className="flex justify-between text-[11px] text-text-secondary">
                    <span>{t("joycon.irTuner.live.barLabel")}</span>
                    <span className="font-mono">{Math.round(thresholdPct)}%</span>
                  </div>
                  <div className="h-2 rounded-full bg-border overflow-hidden">
                    <div
                      className={`h-full transition-all duration-75 ${
                        irLive.proximity_active
                          ? "bg-accent"
                          : "bg-text-secondary/40"
                      }`}
                      style={{ width: `${thresholdPct}%` }}
                    />
                  </div>
                </div>

                {history.length > 1 && (
                  <div className="space-y-1">
                    <p className="text-[11px] text-text-secondary">
                      {t("joycon.irTuner.live.history")}
                    </p>
                    <svg
                      viewBox={`0 0 ${history.length} 32`}
                      className="w-full h-8 text-accent"
                      preserveAspectRatio="none"
                    >
                      <polyline
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="1"
                        points={history
                          .map((v, i) => `${i},${32 - (v / maxHistory) * 30}`)
                          .join(" ")}
                      />
                    </svg>
                  </div>
                )}

                <p className="text-[11px] text-text-secondary">
                  {t("joycon.irTuner.live.hint")}
                </p>
              </>
            )}

            {viewMode === "nfc" && (
              <>
                <p className="text-[11px] font-medium text-text-secondary">
                  {t("joycon.irTuner.nfcLive.sensingZone")}
                </p>
                <div
                  className={`rounded-mac border-2 p-4 text-center transition-colors ${
                    nfcLive.tag_present
                      ? "border-accent bg-accent/10"
                      : readyToScan
                        ? "border-accent/70 bg-accent/5"
                        : scanning
                          ? "border-accent/40 bg-accent/[0.03]"
                          : sessionReady
                          ? "border-border bg-surface-secondary/40"
                          : "border-dashed border-border"
                  }`}
                >
                  <div
                    className={`mx-auto mb-2 w-12 h-12 rounded-full flex items-center justify-center text-lg ${
                      nfcLive.tag_present
                        ? "bg-accent text-white animate-pulse"
                        : readyToScan
                          ? "bg-accent/20 text-accent"
                          : scanning
                            ? "bg-accent/15 text-accent animate-pulse"
                            : sessionReady
                            ? "bg-border text-text-secondary"
                            : "bg-border/50 text-text-secondary"
                    }`}
                  >
                    {nfcLive.tag_present ? "✓" : readyToScan ? "●" : scanning ? "…" : "NFC"}
                  </div>
                  <p
                    className={`text-base font-semibold ${
                      nfcLive.tag_present || readyToScan || scanning
                        ? "text-accent"
                        : "text-text-secondary"
                    }`}
                  >
                    {nfcZoneLabel()}
                  </p>
                  <p className="text-xs text-text-secondary mt-1">
                    {nfcStateLabel(nfcLive.nfc_state)}
                  </p>
                </div>

                <div className="space-y-1">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-xs text-text-secondary">
                      {t("joycon.irTuner.nfcLive.currentUid")}
                    </span>
                    <div className="flex items-center gap-2">
                      {sessionReady && (
                        <button
                          type="button"
                          className="text-[11px] px-2 py-0.5 rounded-mac border border-border hover:bg-surface-secondary"
                          onClick={restartNfcScan}
                        >
                          {t("joycon.irTuner.nfcLive.rescan")}
                        </button>
                      )}
                      {displayUid && (
                        <button
                          type="button"
                          className="text-[11px] px-2 py-0.5 rounded-mac border border-border hover:bg-surface-secondary"
                          onClick={copyUid}
                        >
                          {copiedUid
                            ? t("joycon.irTuner.nfcLive.copied")
                            : t("joycon.irTuner.nfcLive.copy")}
                        </button>
                      )}
                    </div>
                  </div>
                  <p className="font-mono text-lg tracking-wide break-all rounded-mac bg-surface-secondary/60 px-3 py-2 min-h-[2.5rem]">
                    {displayUid || "—"}
                  </p>
                </div>

                <dl className="grid grid-cols-3 gap-x-3 gap-y-2 text-xs">
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.nfcLive.uidLen")}
                    </dt>
                    <dd className="font-mono text-sm">
                      {displayUid ? nfcLive.uid_len : "—"}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.nfcLive.tagType")}
                    </dt>
                    <dd className="font-mono text-sm">
                      {displayUid && nfcLive.tag_type > 0
                        ? `0x${nfcLive.tag_type.toString(16).padStart(2, "0")}`
                        : "—"}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-text-secondary">
                      {t("joycon.irTuner.nfcLive.state")}
                    </dt>
                    <dd className="font-mono text-sm">
                      0x{nfcLive.nfc_state.toString(16).padStart(2, "0")}
                    </dd>
                  </div>
                </dl>

                {uidHistory.length > 0 && (
                  <div className="space-y-1">
                    <p className="text-[11px] text-text-secondary">
                      {t("joycon.irTuner.nfcLive.senseHistory")}
                    </p>
                    <ul className="space-y-1 max-h-24 overflow-y-auto">
                      {uidHistory.map((uid, index) => (
                        <li
                          key={`${uid}-${index}`}
                          className={`font-mono text-[11px] px-2 py-1 rounded break-all ${
                            index === 0
                              ? "bg-surface-secondary/80"
                              : "bg-surface-secondary/40"
                          }`}
                        >
                          {uid}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

                <p className="text-[11px] text-text-secondary">
                  {t("joycon.irTuner.nfcLive.hint")}
                </p>
              </>
            )}
          </div>
        )}

        {mcu.mode === "ir" && (
          <div className="rounded-mac border border-border p-3 text-xs text-text-secondary space-y-1">
            <p>{t("joycon.irTuner.mappingHint")}</p>
            <code className="block font-mono text-[11px] text-text">
              ir_proximity
            </code>
          </div>
        )}

        {mcu.mode === "nfc" && (
          <div className="rounded-mac border border-border p-3 text-xs text-text-secondary space-y-1">
            <p>{t("joycon.irTuner.nfcMappingHint")}</p>
            <code className="block font-mono text-[11px] text-text">
              nfc_tag_present
            </code>
          </div>
        )}
      </div>

      <div className="flex justify-between">
        <button
          className="text-xs px-3 py-1.5 rounded-mac border border-border"
          onClick={reset}
        >
          {t("joycon.irTuner.reset")}
        </button>
        <button
          className="text-sm px-3 py-1.5 rounded-mac bg-accent text-white hover:bg-accent-hover"
          onClick={onClose}
        >
          {t("joycon.irTuner.done")}
        </button>
      </div>
    </div>
  );
};
