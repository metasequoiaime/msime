import { useEffect, useState } from "react";
import { ActionButton } from "./action-button";
import { SwitchRow } from "./switch-row";
import { SelectRow } from "./select-row";
import { TextInputRow } from "./text-input-row";
import { SettingsTextareaField } from "./settings-textarea-field";

export interface CloudCandidateProviderPreferences {
  source: "google" | "custom";
  endpoint: string;
  method: "GET" | "POST";
  headers_json: string;
  body_template: string;
  response_format: "candidates" | "google" | "json_path";
  response_path: string;
  timeout_ms: number;
  cache_enabled: boolean;
}

/** 旧宿主或尚未载入共享默认值时的显示值；有效配置由 Rust 校验。 */
const defaultProvider: CloudCandidateProviderPreferences = {
  source: "google",
  endpoint: "",
  method: "GET",
  headers_json: "{}",
  body_template: '{"query":"{query}"}',
  response_format: "candidates",
  response_path: "data.0",
  timeout_ms: 2000,
  cache_enabled: true,
};

export interface CloudCandidatesSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
  provider?: CloudCandidateProviderPreferences;
  onProviderChange?: (value: CloudCandidateProviderPreferences) => void;
}

/** 共享云候选配置；服务规则、请求构造和响应解析都由 Rust 持有。 */
export function CloudCandidatesSection({
  value,
  onChange,
  provider = defaultProvider,
  onProviderChange,
}: CloudCandidatesSectionProps) {
  const [draft, setDraft] = useState(provider);
  useEffect(() => setDraft(provider), [provider]);
  const change = (patch: Partial<CloudCandidateProviderPreferences>) =>
    setDraft((current) => ({ ...current, ...patch }));
  let recipient = "Google input-tools（inputtools.google.com）";
  if (provider.source === "custom") {
    try {
      recipient = new URL(provider.endpoint.replaceAll("{query}", "query")).hostname;
    } catch {
      recipient = "所选自定义服务";
    }
  }
  return (
    <>
      <SwitchRow
        title="云候选"
        description={`仅将当前正在组的拼写发送给 ${recipient}；不发送已上屏内容、词库或历史`}
        checked={value ?? true}
        onChange={onChange}
      />
      {onProviderChange && (
        <>
          <SelectRow
            title="云候选来源"
            aria-label="云候选来源"
            value={draft.source}
            onChange={(event) => change({ source: event.target.value as "google" | "custom" })}
          >
            <option value="google">Google input-tools</option>
            <option value="custom">自定义 HTTP 接口</option>
          </SelectRow>
          {draft.source === "custom" && (
            <>
              <TextInputRow
                title="接口地址"
                label="云候选接口地址"
                value={draft.endpoint}
                placeholder="https://example.com/candidates?q={query}"
                maxLength={2048}
                description="GET 用 {query} 填入当前拼写；支持 HTTPS 或本机 HTTP"
                onChange={(endpoint) => change({ endpoint })}
              />
              <SelectRow
                title="请求方法"
                aria-label="云候选请求方法"
                value={draft.method}
                onChange={(event) => change({ method: event.target.value as "GET" | "POST" })}
              >
                <option value="GET">GET</option>
                <option value="POST">POST</option>
              </SelectRow>
              <SettingsTextareaField
                label="请求头（JSON）"
                ariaLabel="云候选请求头"
                value={draft.headers_json}
                rows={3}
                maxLength={4096}
                spellCheck={false}
                description='例如 {"Authorization":"Bearer …"}，仅保存在本机'
                onChange={(headers_json) => change({ headers_json })}
              />
              {draft.method === "POST" && (
                <SettingsTextareaField
                  label="请求体模板（JSON）"
                  ariaLabel="云候选请求体模板"
                  value={draft.body_template}
                  rows={4}
                  maxLength={4096}
                  spellCheck={false}
                  description="{query} 仅在 JSON 字符串值中替换，自动转义；可用于自建模型或 OpenAI 兼容接口"
                  onChange={(body_template) => change({ body_template })}
                />
              )}
              <SelectRow
                title="响应格式"
                aria-label="云候选响应格式"
                value={draft.response_format}
                onChange={(event) =>
                  change({
                    response_format: event.target
                      .value as CloudCandidateProviderPreferences["response_format"],
                  })
                }
              >
                <option value="candidates">标准 candidates 数组</option>
                <option value="google">Google input-tools</option>
                <option value="json_path">JSON 路径</option>
              </SelectRow>
              {draft.response_format === "json_path" && (
                <TextInputRow
                  title="JSON 路径"
                  label="云候选 JSON 路径"
                  value={draft.response_path}
                  placeholder="data.0"
                  maxLength={128}
                  description="点分对象键或数组下标，路径最终指向候选文本，例如 choices.0.message.content"
                  onChange={(response_path) => change({ response_path })}
                />
              )}
            </>
          )}
          <TextInputRow
            title="超时（毫秒）"
            label="云候选超时"
            type="number"
            min={100}
            max={10000}
            step={100}
            value={String(draft.timeout_ms)}
            onChange={(raw) => change({ timeout_ms: Number(raw) })}
          />
          <SwitchRow
            title="缓存云候选"
            description="仅在本机内存缓存，按来源隔离；无结果缓存 60 秒"
            checked={draft.cache_enabled}
            onChange={(cache_enabled) => change({ cache_enabled })}
          />
          <ActionButton label="保存云候选服务" action={() => onProviderChange(draft)} />
        </>
      )}
    </>
  );
}
