import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ImuConfig {
  shake_threshold: number;
  flip_threshold: number;
  gesture_cooldown_ms: number;
  gesture_hold_ms: number;
}

interface Props {
  onClose: () => void;
}

const DEFAULTS: ImuConfig = {
  shake_threshold: 28000,
  flip_threshold: 18000,
  gesture_cooldown_ms: 400,
  gesture_hold_ms: 180,
};

export const ImuTuner: React.FC<Props> = ({ onClose }) => {
  const [imu, setImu] = useState<ImuConfig>(DEFAULTS);

  useEffect(() => {
    invoke<ImuConfig>("plugin:joycon|joycon_get_imu")
      .then(setImu)
      .catch(() => setImu(DEFAULTS));
  }, []);

  const update = (patch: Partial<ImuConfig>) => {
    const next = { ...imu, ...patch };
    setImu(next);
    invoke("plugin:joycon|joycon_set_imu", { imu: next }).catch(() => {});
  };

  const reset = async () => {
    setImu(DEFAULTS);
    await invoke("plugin:joycon|joycon_set_imu", { imu: DEFAULTS }).catch(() => {});
  };

  const Slider: React.FC<{
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    onChange: (v: number) => void;
    hint?: string;
  }> = ({ label, value, min, max, step, onChange, hint }) => (
    <div className="space-y-1">
      <div className="flex justify-between text-sm">
        <label>{label}</label>
        <span className="font-mono text-xs text-text-secondary">{value}</span>
      </div>
      <input
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(parseInt(e.target.value, 10))}
        className="w-full"
      />
      {hint && <p className="text-xs text-text-secondary">{hint}</p>}
    </div>
  );

  return (
    <div className="max-w-2xl w-full mx-auto space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold">IMU Gesture Tuning</h1>
        <button
          className="text-xs text-text-secondary hover:text-text"
          onClick={onClose}
        >
          ← Back
        </button>
      </div>

      <div className="surface-card p-4 space-y-4">
        <Slider
          label="Shake threshold"
          value={imu.shake_threshold}
          min={10000}
          max={50000}
          step={500}
          onChange={(v) => update({ shake_threshold: v })}
          hint="Lower = more sensitive. Default 28000."
        />
        <Slider
          label="Flip threshold (gyro)"
          value={imu.flip_threshold}
          min={5000}
          max={40000}
          step={500}
          onChange={(v) => update({ flip_threshold: v })}
          hint="Lower = more sensitive. Default 18000."
        />
        <Slider
          label="Gesture cooldown (ms)"
          value={imu.gesture_cooldown_ms}
          min={100}
          max={1500}
          step={50}
          onChange={(v) => update({ gesture_cooldown_ms: v })}
          hint="Min interval between same gesture firing twice."
        />
        <Slider
          label="Gesture hold (ms)"
          value={imu.gesture_hold_ms}
          min={50}
          max={500}
          step={20}
          onChange={(v) => update({ gesture_hold_ms: v })}
          hint="How long the virtual button stays pressed once gesture fires."
        />
      </div>

      <div className="flex justify-between">
        <button
          className="text-xs px-3 py-1.5 rounded-mac border border-border"
          onClick={reset}
        >
          Reset to Defaults
        </button>
        <button
          className="text-sm px-3 py-1.5 rounded-mac bg-accent text-white hover:bg-accent-hover"
          onClick={onClose}
        >
          Done
        </button>
      </div>
    </div>
  );
};
