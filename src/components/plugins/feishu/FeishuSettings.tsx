import React, { useState } from "react";
import { useSettings } from "../../../hooks/useSettings";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { Input } from "../../ui/Input";
import { Button } from "../../ui/Button";

interface FeishuConfig {
  enabled: boolean;
  app_id: string;
  app_secret: string;
  document_id: string;
  prepend_timestamp: boolean;
  folder_token: string;
}

export const FeishuSettings: React.FC = () => {
  const { settings, updateSetting } = useSettings();
  const feishu: FeishuConfig = (settings as any)?.feishu ?? {
    enabled: false,
    app_id: "",
    app_secret: "",
    document_id: "",
    prepend_timestamp: true,
    folder_token: "",
  };
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);

  const updateFeishu = (patch: Partial<FeishuConfig>) => {
    (updateSetting as any)("feishu", { ...feishu, ...patch });
  };

  const extractDocId = (input: string): string => {
    const match = input.match(/\/docx\/([A-Za-z0-9]+)/);
    return match ? match[1] : input;
  };

  return (
    <div className="space-y-4">
      <SettingsGroup title="飞书文档集成">
        <ToggleSwitch
          label="启用飞书"
          description="转录完成后自动追加到飞书文档"
          checked={feishu.enabled}
          onChange={(v) => updateFeishu({ enabled: v })}
          grouped={true}
        />

        <SettingContainer
          title="App ID"
          description="飞书开放平台应用 ID"
          grouped={true}
        >
          <Input
            value={feishu.app_id}
            onChange={(e) => updateFeishu({ app_id: e.target.value })}
            placeholder="cli_xxxxxxxxxx"
            className="w-56"
          />
        </SettingContainer>

        <SettingContainer
          title="App Secret"
          description="飞书开放平台应用密钥"
          grouped={true}
        >
          <Input
            type="password"
            value={feishu.app_secret}
            onChange={(e) => updateFeishu({ app_secret: e.target.value })}
            placeholder="xxxxxxxxxxxxxxxx"
            className="w-56"
          />
        </SettingContainer>

        <SettingContainer
          title="文档 ID"
          description="目标文档 ID 或完整 URL（自动提取）"
          grouped={true}
        >
          <Input
            value={feishu.document_id}
            onChange={(e) =>
              updateFeishu({ document_id: extractDocId(e.target.value) })
            }
            placeholder="doxcnXXXXXXXX 或粘贴文档链接"
            className="w-72"
          />
        </SettingContainer>

        <ToggleSwitch
          label="添加时间戳"
          description="每条转录前加 [HH:MM:SS] 时间标记"
          checked={feishu.prepend_timestamp}
          onChange={(v) => updateFeishu({ prepend_timestamp: v })}
          grouped={true}
        />

        <SettingContainer
          title="文件夹 Token（可选）"
          description="新建文档时的目标文件夹"
          grouped={true}
        >
          <Input
            value={feishu.folder_token}
            onChange={(e) => updateFeishu({ folder_token: e.target.value })}
            placeholder="留空使用根目录"
            className="w-56"
          />
        </SettingContainer>
      </SettingsGroup>

      {feishu.enabled && feishu.app_id && feishu.app_secret && (
        <div className="flex items-center gap-3 px-1">
          <Button
            variant="secondary"
            size="sm"
            onClick={async () => {
              setTesting(true);
              setTestResult(null);
              try {
                const res = await fetch(
                  "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal",
                  {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({
                      app_id: feishu.app_id,
                      app_secret: feishu.app_secret,
                    }),
                  },
                );
                const data = await res.json();
                if (data.code === 0) {
                  setTestResult("✓ 认证成功");
                } else {
                  setTestResult(`✗ ${data.msg}`);
                }
              } catch (e: any) {
                setTestResult(`✗ ${e.message || "连接失败"}`);
              } finally {
                setTesting(false);
              }
            }}
            disabled={testing}
          >
            {testing ? "测试中..." : "测试连接"}
          </Button>
          {testResult && (
            <span
              className={`text-xs ${testResult.startsWith("✓") ? "text-green-500" : "text-red-500"}`}
            >
              {testResult}
            </span>
          )}
        </div>
      )}
    </div>
  );
};
