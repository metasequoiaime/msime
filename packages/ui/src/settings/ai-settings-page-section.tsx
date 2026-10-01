import type { ReactNode } from "react";
import { clamp } from "../core/number";
import { GroupList, MoreOptions, Row, Select, Switch } from "../core/platform-controls";
import { AiBasicSettingsSection, type AiProviderOption } from "./ai-basic-settings-section";
import { AiCandidateLimitSection } from "./ai-candidate-limit-section";
import { AiModelCatalogSection } from "./ai-model-catalog-section";
import { AiPromptSettingsSection } from "./ai-prompt-settings-section";
import { CustomPromptSlotOptions } from "./custom-prompt-slot-options";
import { ModelSelect } from "./model-select";
import { SettingsTextareaField } from "./settings-textarea-field";
import * as settings from "./settings-style";

export interface AiSettingsPageSectionProps {
  /** 设置窗口的 AI 辅助页为真，画成 服务 → 联想 → 提示词 → 测试工具 几组；旧的嵌入式面板不传，保持原来的卡片。 */
  grouped?: boolean;
  /** AI 服务的接口地址是否有效；无效时「更多选项」首次显示就展开，因为凭据要等接口地址填对才能用。 */
  endpointValid?: boolean;
  disabled: boolean;
  hidden: boolean;
  enabled: boolean;
  enabledDescription: string;
  provider: string;
  providerOptions: readonly AiProviderOption[];
  model: string;
  endpoint: string;
  providerPreset: ReactNode;
  onEnabledChange: (enabled: boolean) => void;
  onProviderChange: (provider: string) => void;
  onModelChange: (model: string) => void;
  onEndpointChange: (endpoint: string) => void;
  credentialSection: ReactNode;
  /** 测试当前配置的按钮，放在服务配置的末尾。 */
  desktopCredentialTest: ReactNode;
  modelCatalog: {
    busy: boolean;
    origin: string;
    models: string[] | undefined;
    status: string;
    onFetch: () => void;
    onSelect: (model: string) => void;
  } | null;
  candidateLimit: number;
  onCandidateLimitChange: (candidateLimit: number) => void;
  promptId?: string;
  prompt?: string;
  promptCustom1: string;
  promptCustom2: string;
  promptCustom3: string;
  fallbackPrompt: string;
  onPromptIdChange: (promptId: string | undefined) => void;
  onPromptChange: (prompt: string | undefined) => void;
  onPromptCustom1Change: (value: string) => void;
  onPromptCustom2Change: (value: string) => void;
  onPromptCustom3Change: (value: string) => void;
  testTools: ReactNode;
  mcpConnect: ReactNode;
}

/** Page-level composition for the shared AI assistant settings. */
export function AiSettingsPageSection({
  grouped = false,
  endpointValid = true,
  disabled,
  hidden,
  enabled,
  enabledDescription,
  provider,
  providerOptions,
  model,
  endpoint,
  providerPreset,
  onEnabledChange,
  onProviderChange,
  onModelChange,
  onEndpointChange,
  credentialSection,
  desktopCredentialTest,
  modelCatalog,
  candidateLimit,
  onCandidateLimitChange,
  promptId,
  prompt,
  promptCustom1,
  promptCustom2,
  promptCustom3,
  fallbackPrompt,
  onPromptIdChange,
  onPromptChange,
  onPromptCustom1Change,
  onPromptCustom2Change,
  onPromptCustom3Change,
  testTools,
  mcpConnect,
}: AiSettingsPageSectionProps) {
  if (grouped) {
    const promptSlot = promptId === "custom" ? "custom_1" : promptId || "custom_1";
    const slotPrompts = {
      custom_1: { label: "自定义提示词一", value: promptCustom1, onChange: onPromptCustom1Change },
      custom_2: { label: "自定义提示词二", value: promptCustom2, onChange: onPromptCustom2Change },
      custom_3: { label: "自定义提示词三", value: promptCustom3, onChange: onPromptCustom3Change },
    } as const;
    const slot = slotPrompts[promptSlot as keyof typeof slotPrompts] ?? slotPrompts.custom_1;
    const compatiblePrompt = prompt ?? fallbackPrompt;
    return (
      <fieldset disabled={disabled} hidden={hidden} aria-label="AI 辅助">
        <div className={settings.groups}>
          {/* 先把服务接通：开关、服务商、凭据、模型；接口地址一般随服务商预设，收进「更多选项」；测试按钮放在组末，测的正是上面这些。 */}
          <GroupList title="服务">
            <Row title="启用 AI 辅助" description={enabledDescription}>
              <Switch aria-label="启用 AI 辅助" checked={enabled} onChange={onEnabledChange} />
            </Row>
            <Row title="服务提供商">
              <Select
                aria-label="AI 服务提供商"
                value={provider}
                onChange={(event) => onProviderChange(event.target.value)}
              >
                {providerOptions.map((option) => (
                  <option key={option.id} value={option.id}>
                    {option.title}
                  </option>
                ))}
              </Select>
            </Row>
            {credentialSection}
            {providerPreset}
            <Row title="模型">
              <input
                aria-label="AI 模型"
                value={model}
                onChange={(event) => onModelChange(event.target.value)}
              />
            </Row>
            {modelCatalog && (
              <>
                <Row
                  title="服务模型"
                  description="从当前服务的模型目录读取；服务不支持时可继续手动填写模型。"
                >
                  <button
                    type="button"
                    className="secondary"
                    disabled={modelCatalog.busy || !modelCatalog.origin}
                    onClick={modelCatalog.onFetch}
                  >
                    {modelCatalog.busy ? "获取中…" : "获取模型列表"}
                  </button>
                </Row>
                {modelCatalog.models && modelCatalog.models.length > 0 && (
                  <Row title="已获取模型">
                    <ModelSelect
                      models={modelCatalog.models}
                      model={model}
                      ariaLabel="已获取的 AI 模型"
                      emptyLabel="选择模型…"
                      onSelect={modelCatalog.onSelect}
                    />
                  </Row>
                )}
                {modelCatalog.status && (
                  <p className={settings.groupNote} role="status">
                    {modelCatalog.status}
                  </p>
                )}
              </>
            )}
            <MoreOptions defaultOpen={!endpointValid}>
              <Row title="接口地址">
                <input
                  aria-label="AI 接口地址"
                  type="url"
                  value={endpoint}
                  onChange={(event) => onEndpointChange(event.target.value)}
                />
              </Row>
            </MoreOptions>
            {desktopCredentialTest && (
              <div className={settings.groupBlock}>{desktopCredentialTest}</div>
            )}
          </GroupList>
          <GroupList title="联想">
            <Row title="候选数量" description="每次 AI 联想给出的候选个数，1 到 10">
              <input
                aria-label="AI 候选数量"
                type="number"
                min="1"
                max="10"
                value={candidateLimit}
                onChange={(event) =>
                  onCandidateLimitChange(clamp(Number(event.target.value) || 3, 1, 10))
                }
              />
            </Row>
          </GroupList>
          {/* 只显示所选槽位的提示词；兼容提示词是旧版留下的后备，只在所选槽位留空时使用，收进「更多选项」。 */}
          <GroupList title="提示词">
            <Row title="提示词方案" description="使用选中的独立槽位；槽位留空时使用兼容提示词">
              <Select
                aria-label="AI 联想提示词方案"
                value={promptSlot}
                onChange={(event) => onPromptIdChange(event.target.value)}
              >
                <CustomPromptSlotOptions />
              </Select>
            </Row>
            <div className={settings.managerBlock}>
              <SettingsTextareaField
                label={slot.label}
                description="发送给 AI 联想服务的额外提示词"
                ariaLabel={slot.label}
                value={slot.value}
                onChange={slot.onChange}
              />
            </div>
            <MoreOptions>
              <div className={settings.managerBlock}>
                <SettingsTextareaField
                  label="兼容提示词"
                  description="旧版提示词，所选自定义槽位留空时使用"
                  ariaLabel="AI 润色提示词"
                  placeholder="留空时使用内置的联想提示词"
                  value={compatiblePrompt}
                  onChange={onPromptChange}
                />
                <div className={settings.managerActions}>
                  <button
                    type="button"
                    className="secondary"
                    disabled={compatiblePrompt === fallbackPrompt}
                    onClick={() => onPromptChange(fallbackPrompt)}
                  >
                    恢复默认
                  </button>
                </div>
              </div>
            </MoreOptions>
          </GroupList>
          {testTools}
          {mcpConnect}
        </div>
      </fieldset>
    );
  }
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="AI 辅助">
      <AiBasicSettingsSection
        enabled={enabled}
        enabledDescription={enabledDescription}
        provider={provider}
        providerOptions={providerOptions}
        model={model}
        endpoint={endpoint}
        providerPreset={providerPreset}
        onEnabledChange={onEnabledChange}
        onProviderChange={onProviderChange}
        onModelChange={onModelChange}
        onEndpointChange={onEndpointChange}
      />
      {credentialSection}
      {desktopCredentialTest}
      {modelCatalog && (
        <AiModelCatalogSection
          busy={modelCatalog.busy}
          origin={modelCatalog.origin}
          models={modelCatalog.models}
          selectedModel={model}
          status={modelCatalog.status}
          onFetch={modelCatalog.onFetch}
          onSelect={modelCatalog.onSelect}
        />
      )}
      <AiCandidateLimitSection value={candidateLimit} onChange={onCandidateLimitChange} />
      <AiPromptSettingsSection
        promptId={promptId}
        prompt={prompt}
        promptCustom1={promptCustom1}
        promptCustom2={promptCustom2}
        promptCustom3={promptCustom3}
        fallbackPrompt={fallbackPrompt}
        onPromptIdChange={onPromptIdChange}
        onPromptChange={onPromptChange}
        onPromptCustom1Change={onPromptCustom1Change}
        onPromptCustom2Change={onPromptCustom2Change}
        onPromptCustom3Change={onPromptCustom3Change}
      />
      {testTools}
      {mcpConnect}
    </fieldset>
  );
}
