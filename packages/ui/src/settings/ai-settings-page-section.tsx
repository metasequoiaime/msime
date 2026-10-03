import { SettingsGroupNote } from "./settings-group-note";
import type { ReactNode } from "react";
import { clamp } from "../core/number";
import { GroupList, MoreOptions, Row } from "../core/platform-controls";
import { CustomPromptSlotOptions } from "./custom-prompt-slot-options";
import { ModelSelect } from "./model-select";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SwitchRow } from "./switch-row";
import { SelectRow } from "./select-row";
import { TextInputRow } from "./text-input-row";
import { ActionRow } from "./action-row";
import { SettingsPageFieldset } from "./settings-page-fieldset";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsManagerBlock } from "./settings-manager-block";

export type AiProviderOption = { id: string; title: string };

export interface AiSettingsPageSectionProps {
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
  promptCustom1: string;
  promptCustom2: string;
  promptCustom3: string;
  onPromptIdChange: (promptId: string | undefined) => void;
  onPromptCustom1Change: (value: string) => void;
  onPromptCustom2Change: (value: string) => void;
  onPromptCustom3Change: (value: string) => void;
  testTools: ReactNode;
  /** 最后一组之后的内容：设置页在这里放进入「AI 对话」的入口。 */
  trailing: ReactNode;
}

/** Page-level composition for the shared AI assistant settings: the 服务, 联想, 提示词 and 测试工具 groups. */
export function AiSettingsPageSection({
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
  promptCustom1,
  promptCustom2,
  promptCustom3,
  onPromptIdChange,
  onPromptCustom1Change,
  onPromptCustom2Change,
  onPromptCustom3Change,
  testTools,
  trailing,
}: AiSettingsPageSectionProps) {
  const promptSlot = promptId || "custom_1";
  const slotPrompts = {
    custom_1: { label: "自定义提示词一", value: promptCustom1, onChange: onPromptCustom1Change },
    custom_2: { label: "自定义提示词二", value: promptCustom2, onChange: onPromptCustom2Change },
    custom_3: { label: "自定义提示词三", value: promptCustom3, onChange: onPromptCustom3Change },
  } as const;
  const slot = slotPrompts[promptSlot as keyof typeof slotPrompts] ?? slotPrompts.custom_1;
  return (
    <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="AI 辅助">
      {/* 先把服务接通：开关、服务商、凭据、模型；接口地址一般随服务商预设，收进「更多选项」；测试按钮放在组末，测的正是上面这些。 */}
      <GroupList title="服务">
        <SwitchRow
          title="启用 AI 辅助"
          description={enabledDescription}
          aria-label="启用 AI 辅助"
          checked={enabled}
          onChange={onEnabledChange}
        />
        <SelectRow
          title="服务提供商"
          aria-label="AI 服务提供商"
          value={provider}
          onChange={(event) => onProviderChange(event.target.value)}
        >
          {providerOptions.map((option) => (
            <option key={option.id} value={option.id}>
              {option.title}
            </option>
          ))}
        </SelectRow>
        {credentialSection}
        {providerPreset}
        <TextInputRow title="模型" label="AI 模型" value={model} onChange={onModelChange} />
        {modelCatalog && (
          <>
            <ActionRow
              title="服务模型"
              description="从当前服务的模型目录读取；服务不支持时可继续手动填写模型。"
              action={modelCatalog.onFetch}
              disabled={modelCatalog.busy || !modelCatalog.origin}
              label={modelCatalog.busy ? "获取中…" : "获取模型列表"}
            />
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
              <SettingsGroupNote role="status">{modelCatalog.status}</SettingsGroupNote>
            )}
          </>
        )}
        <MoreOptions defaultOpen={!endpointValid}>
          <TextInputRow
            title="接口地址"
            label="AI 接口地址"
            type="url"
            value={endpoint}
            onChange={onEndpointChange}
          />
        </MoreOptions>
        {desktopCredentialTest && <SettingsGroupBlock>{desktopCredentialTest}</SettingsGroupBlock>}
      </GroupList>
      <GroupList title="联想">
        <TextInputRow
          title="候选数量"
          description="每次 AI 联想给出的候选个数，1 到 10"
          label="AI 候选数量"
          type="number"
          min="1"
          max="10"
          value={String(candidateLimit)}
          onChange={(value) => onCandidateLimitChange(clamp(Number(value) || 3, 1, 10))}
        />
      </GroupList>
      {/* 只显示所选槽位的提示词；槽位留空时使用内置的联想提示词。 */}
      <GroupList title="提示词">
        <SelectRow
          title="提示词方案"
          description="使用选中的独立槽位；槽位留空时使用内置的联想提示词"
          aria-label="AI 联想提示词方案"
          value={promptSlot}
          onChange={(event) => onPromptIdChange(event.target.value)}
        >
          <CustomPromptSlotOptions />
        </SelectRow>
        <SettingsManagerBlock>
          <SettingsTextareaField
            label={slot.label}
            description="发送给 AI 联想服务的额外提示词"
            ariaLabel={slot.label}
            value={slot.value}
            onChange={slot.onChange}
          />
        </SettingsManagerBlock>
      </GroupList>
      {testTools}
      {trailing}
    </SettingsPageFieldset>
  );
}
