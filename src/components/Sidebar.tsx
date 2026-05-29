import React from "react";
import { useTranslation } from "react-i18next";
import { Cog, FlaskConical, History, Info, Sparkles, Cpu, Puzzle } from "lucide-react";
import JoyTalkTextLogo from "./icons/JoyTalkTextLogo";
import JoyTalkLogo from "./icons/JoyTalkLogo";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
} from "./settings";
import { PluginsPage } from "./plugins/PluginsPage";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  general: {
    labelKey: "sidebar.general",
    icon: JoyTalkLogo,
    component: GeneralSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  plugins: {
    labelKey: "sidebar.plugins",
    icon: Puzzle,
    component: PluginsPage,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (settings) => settings?.post_process_enabled ?? false,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <aside
      className="flex flex-col w-44 h-full surface-2 border-r border-border select-none"
      style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
    >
      <div className="h-7" /> {/* traffic-light spacer */}
      <div className="px-3 pb-3 flex items-center gap-2">
        <JoyTalkLogo width={22} height={22} />
        <JoyTalkTextLogo width={86} className="text-text" />
      </div>
      <nav
        className="flex flex-col gap-[1px] px-2 pt-2 border-t border-border"
        style={{ WebkitAppRegion: "no-drag" } as React.CSSProperties}
      >
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <button
              key={section.id}
              type="button"
              onClick={() => onSectionChange(section.id)}
              className={`flex gap-2 items-center px-2.5 py-[6px] w-full rounded-md text-left transition-colors ${
                isActive
                  ? "bg-accent/15 text-accent"
                  : "text-text hover:bg-text/[0.04]"
              }`}
              title={t(section.labelKey)}
            >
              <Icon
                width={15}
                height={15}
                className={`shrink-0 ${isActive ? "text-accent" : "text-text-secondary"}`}
              />
              <span className="text-[13px] font-medium truncate">
                {t(section.labelKey)}
              </span>
            </button>
          );
        })}
      </nav>
    </aside>
  );
};
