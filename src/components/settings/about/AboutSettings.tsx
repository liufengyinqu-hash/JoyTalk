import React, { useState, useEffect } from "react";
import { Coffee } from "lucide-react";
import { useTranslation } from "react-i18next";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { Button } from "../../ui/Button";
import { AppDataDirectory } from "../AppDataDirectory";
import { AppLanguageSelector } from "../AppLanguageSelector";
import { LogDirectory } from "../debug";

const ALIPAY_QR_SRC = "/donate/alipay-qr.png";

export const AboutSettings: React.FC = () => {
  const { t } = useTranslation();
  const [version, setVersion] = useState("");
  const [showDonateModal, setShowDonateModal] = useState(false);
  const [isChecking, setIsChecking] = useState(false);
  const [updateStatus, setUpdateStatus] = useState<"idle" | "available" | "upToDate">("idle");

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("0.1.2");
      }
    };

    fetchVersion();
  }, []);

  const handleCheckUpdate = async () => {
    setIsChecking(true);
    setUpdateStatus("idle");
    try {
      const update = await check();
      if (update) {
        setUpdateStatus("available");
        await update.downloadAndInstall();
        await relaunch();
      } else {
        setUpdateStatus("upToDate");
        setTimeout(() => setUpdateStatus("idle"), 3000);
      }
    } catch (e) {
      console.error("Update check failed:", e);
      setUpdateStatus("idle");
    } finally {
      setIsChecking(false);
    }
  };

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.about.title")}>
        <AppLanguageSelector descriptionMode="tooltip" grouped={true} />
        <SettingContainer
          title={t("settings.about.version.title")}
          description={t("settings.about.version.description")}
          grouped={true}
        >
          <div className="flex items-center gap-3">
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-sm font-mono">v{version}</span>
            <Button
              variant="secondary"
              size="sm"
              onClick={handleCheckUpdate}
              disabled={isChecking}
            >
              {isChecking
                ? "检查中..."
                : updateStatus === "upToDate"
                  ? "已是最新 ✓"
                  : updateStatus === "available"
                    ? "更新中..."
                    : "检查更新"}
            </Button>
          </div>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.supportDevelopment.title")}
          description={t("settings.about.supportDevelopment.description")}
          grouped={true}
        >
          <Button
            variant="secondary"
            size="md"
            onClick={() => setShowDonateModal(true)}
            className="inline-flex items-center gap-2"
          >
            <Coffee className="h-4 w-4 text-amber-600" aria-hidden="true" />
            {t("settings.about.supportDevelopment.button")}
          </Button>
        </SettingContainer>

        <SettingContainer
          title={t("settings.about.sourceCode.title")}
          description={t("settings.about.sourceCode.description")}
          grouped={true}
        >
          <Button
            variant="secondary"
            size="md"
            onClick={() => openUrl("https://github.com/liufengyinqu-hash/JoyTalk")}
          >
            {t("settings.about.sourceCode.button")}
          </Button>
        </SettingContainer>

        <AppDataDirectory descriptionMode="tooltip" grouped={true} />
        <LogDirectory grouped={true} />
      </SettingsGroup>

      {showDonateModal && (
        <div
          className="fixed inset-0 bg-black/40 flex items-center justify-center z-50"
          onClick={() => setShowDonateModal(false)}
        >
          <div
            className="surface-card max-w-sm w-full mx-4 p-5 flex flex-col items-center gap-3"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="w-full flex items-center justify-between">
              <h3 className="text-base font-semibold inline-flex items-center gap-2">
                <Coffee className="h-5 w-5 text-amber-600" aria-hidden="true" />
                {t("settings.about.supportDevelopment.title")}
              </h3>
              <button
                type="button"
                className="text-xs text-text-secondary hover:text-text"
                onClick={() => setShowDonateModal(false)}
              >
                {t("common.close")}
              </button>
            </div>
            <img
              src={ALIPAY_QR_SRC}
              alt={t("settings.about.supportDevelopment.alipayAlt")}
              className="w-full max-w-[240px] h-auto rounded-xl border border-border bg-surface object-contain"
            />
            <p className="text-sm text-text-secondary text-center">
              {t("settings.about.supportDevelopment.alipayCaption")}
            </p>
          </div>
        </div>
      )}

      <SettingsGroup title={t("settings.about.acknowledgments.title")}>
        <SettingContainer
          title={t("settings.about.acknowledgments.whisper.title")}
          description={t("settings.about.acknowledgments.whisper.description")}
          grouped={true}
          layout="stacked"
        >
          <div className="text-sm text-mid-gray">
            {t("settings.about.acknowledgments.whisper.details")}
          </div>
        </SettingContainer>
      </SettingsGroup>
    </div>
  );
};
