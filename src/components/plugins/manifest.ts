export type PluginCategory = "input" | "audio" | "post" | "integration";

export interface PluginManifest {
  id: string;
  name: string;
  description: string;
  version: string;
  author: string;
  icon: string;
  category: PluginCategory;
  capabilities: string[];
  builtin: boolean;
  comingSoon?: boolean;
}

export const BUILTIN_PLUGINS: PluginManifest[] = [
  {
    id: "joycon",
    name: "Joy-Con Controller",
    description:
      "Map Nintendo Switch Joy-Con buttons to actions and keyboard macros",
    version: "0.1.0",
    author: "JoyTalk",
    icon: "🎮",
    category: "input",
    capabilities: ["button-mapping", "macro", "preset"],
    builtin: true,
  },
  {
    id: "feishu",
    name: "飞书文档",
    description: "转录完成后自动追加到飞书文档（会议纪要场景）",
    version: "0.1.0",
    author: "JoyTalk",
    icon: "📄",
    category: "integration",
    capabilities: ["document-append"],
    builtin: true,
  },
  {
    id: "dji-mic",
    name: "DJI Mic Auto-select",
    description: "Auto-switch input device when DJI Mic is connected",
    version: "0.0.0",
    author: "JoyTalk",
    icon: "🎙",
    category: "audio",
    capabilities: ["audio-input"],
    builtin: false,
    comingSoon: true,
  },
  {
    id: "ai-postprocess",
    name: "AI Post-process",
    description: "Clean and polish transcript with LLM",
    version: "0.0.0",
    author: "JoyTalk",
    icon: "✨",
    category: "post",
    capabilities: ["llm"],
    builtin: false,
    comingSoon: true,
  },
];
