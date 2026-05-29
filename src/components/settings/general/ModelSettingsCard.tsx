import React from "react";
import { useTranslation } from "react-i18next";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { LanguageSelector } from "../LanguageSelector";
import { TranslateToEnglish } from "../TranslateToEnglish";
import { useModelStore } from "../../../stores/modelStore";
import { LANGUAGES } from "../../../lib/constants/languages";
import type { ModelInfo } from "@/bindings";

export const ModelSettingsCard: React.FC = () => {
  const { t } = useTranslation();
  const { currentModel, models } = useModelStore();

  const currentModelInfo = models.find((m: ModelInfo) => m.id === currentModel);

  if (!currentModel || !currentModelInfo) {
    return null;
  }

  const supportsLanguageSelection =
    currentModelInfo.supports_language_selection ?? false;
  const supportsTranslation = currentModelInfo.supports_translation ?? false;
  const supported = currentModelInfo.supported_languages ?? [];

  const supportedLabels = supported
    .map((code) => LANGUAGES.find((l) => l.value === code)?.label ?? code)
    .slice(0, 12);
  const overflow = supported.length > 12 ? supported.length - 12 : 0;

  return (
    <SettingsGroup
      title={t("settings.modelSettings.title", {
        model: currentModelInfo.name,
      })}
    >
      {supportsLanguageSelection ? (
        <LanguageSelector
          descriptionMode="tooltip"
          grouped={true}
          supportedLanguages={currentModelInfo.supported_languages}
        />
      ) : (
        <div className="px-3 py-2 text-sm opacity-70">
          <span className="font-medium">Languages:</span>{" "}
          {supported.length === 0
            ? "—"
            : supportedLabels.join(", ") +
              (overflow > 0 ? ` (+${overflow} more)` : "")}
          {supported.length === 1 && (
            <span className="ml-2 text-xs opacity-60">
              (model is single-language; switch model to change language)
            </span>
          )}
        </div>
      )}
      {supportsTranslation && (
        <TranslateToEnglish descriptionMode="tooltip" grouped={true} />
      )}
    </SettingsGroup>
  );
};
