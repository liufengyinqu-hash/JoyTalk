import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  MicrophoneIcon,
  TranscriptionIcon,
  CancelIcon,
} from "../components/icons";
import "./RecordingOverlay.css";
import { commands } from "@/bindings";
import i18n, { syncLanguageFromSettings } from "@/i18n";
import { getLanguageDirection } from "@/lib/utils/rtl";

type OverlayState = "recording" | "transcribing" | "processing";

interface FrontmostApp {
  name: string;
  bundle_id: string;
}

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [levels, setLevels] = useState<number[]>(Array(16).fill(0));
  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  const direction = getLanguageDirection(i18n.language);
  const [appName, setAppName] = useState<string>("");

  useEffect(() => {
    const setupEventListeners = async () => {
      const unlistenShow = await listen("show-overlay", async (event) => {
        await syncLanguageFromSettings();
        const overlayState = event.payload as OverlayState;
        setState(overlayState);
        setIsVisible(true);
        try {
          const f = await invoke<FrontmostApp | null>(
            "plugin:joycon|joycon_get_frontmost_app",
          );
          if (f && f.name) setAppName(f.name);
        } catch {
          // ignore
        }
      });

      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
      });

      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3;
        });
        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, 9));
      });

      const unlistenFront = await listen<string | null>(
        "joycon://frontmost_changed",
        async () => {
          try {
            const f = await invoke<FrontmostApp | null>(
              "plugin:joycon|joycon_get_frontmost_app",
            );
            if (f && f.name) setAppName(f.name);
          } catch {
            // ignore
          }
        },
      );

      return () => {
        unlistenShow();
        unlistenHide();
        unlistenLevel();
        unlistenFront();
      };
    };

    setupEventListeners();
  }, []);

  const getIcon = () => {
    if (state === "recording") {
      return <MicrophoneIcon width={36} height={36} color="#22D3EE" className="overlay-icon" />;
    } else {
      return <TranscriptionIcon width={36} height={36} color="#22D3EE" className="overlay-icon" />;
    }
  };

  return (
    <div
      dir={direction}
      className={`recording-overlay ${isVisible ? "fade-in" : ""}`}
    >
      <div className="overlay-left">{getIcon()}</div>

      <div className="overlay-middle">
        {state === "recording" && (
          <div className="recording-content">
            {appName && <div className="app-tag">{appName}</div>}
            <div className="bars-container">
              {levels.map((v, i) => (
                <div
                  key={i}
                  className="bar"
                  style={{
                    height: `${Math.min(20, 4 + Math.pow(v, 0.7) * 16)}px`,
                    transition:
                      "height 60ms ease-out, opacity 120ms ease-out",
                    opacity: Math.max(0.2, v * 1.7),
                  }}
                />
              ))}
            </div>
          </div>
        )}
        {state === "transcribing" && (
          <div className="transcribing-text">{t("overlay.transcribing")}</div>
        )}
        {state === "processing" && (
          <div className="transcribing-text">{t("overlay.processing")}</div>
        )}
      </div>

      <div className="overlay-right">
        {state === "recording" && (
          <div
            className="cancel-button"
            onClick={() => {
              commands.cancelOperation();
            }}
          >
            <CancelIcon />
          </div>
        )}
      </div>
    </div>
  );
};

export default RecordingOverlay;
