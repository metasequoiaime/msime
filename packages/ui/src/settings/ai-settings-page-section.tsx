import { SettingsGroupNote } from "./settings-group-note";
import type { ReactNode } from "react";
import { clamp } from "../core/number";
import { GroupList, MoreOptions } from "../core/platform-controls";
import { ModelPickerRows } from "./model-picker-rows";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SwitchRow } from "./switch-row";
import { SelectRow } from "./select-row";
import { TextInputRow } from "./text-input-row";
import { ActionRow } from "./action-row";
import { SettingsPageFieldset } from "./settings-page-fieldset";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsManagerBlock } from "./settings-manager-block";
import type { AiProviderOption } from "./ai-provider-options";

export type { AiProviderOption } from "./ai-provider-options";

export interface AiSettingsPageSectionProps {
  /** AI 服务的接口地址是否有效；无效时「更多选项」首次显示就展开，因为凭据要等接口地址填对才能用。 */
  endpointValid?: boolean;
  /** 接口地址不能用时显示在地址栏下方的说明，例如公网 http 为什么被拒绝；地址可用时为空。 */
  endpointHint?: string;
  disabled: boolean;
  hidden: boolean;
  enabled: boolean;
  enabledDescription: string;
  provider: string;
  providerOptions: readonly AiProviderOption[];
  model: string;
  /** 服务商预置的模型，和获取到的模型一起列在「模型」下拉里。 */
  presetModels: readonly string[];
  endpoint: string;
  /** 服务商的接入说明按钮。 */
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
  } | null;
  /** 宿主按偏好决定 AI 候选数量，所以给出「候选数量」；固定数量的宿主（macOS、Windows、Linux、鸿蒙，`HostCapabilities.fixed_ai_candidate_limit`）不给，整个「联想」组随之不出现。 */
  candidateLimitShown: boolean;
  candidateLimit: number;
  onCandidateLimitChange: (candidateLimit: number) => void;
  /** 当前的提示词槽位（`prompt_id`）。页面只编辑这一个槽位，不再提供切换：留空的槽位回退到内置提示词，切换三个槽位几乎没人用。 */
  promptId?: string;
  promptCustom1: string;
  promptCustom2: string;
  promptCustom3: string;
  onPromptCustom1Change: (value: string) => void;
  onPromptCustom2Change: (value: string) => void;
  onPromptCustom3Change: (value: string) => void;
  /** 「AI 润色测试」：只有用这份配置润色的宿主（iOS、Android、HarmonyOS 手机与 2-in-1）传入，macOS、Windows、Linux 上它测的润色提示词和用户实际用到的联想无关。 */
  testTools: ReactNode;
  /** 最后一组之后的内容：设置页在这里放进入「AI 对话」的入口。 */
  trailing: ReactNode;
}

/** 共享 AI 辅助设置的页面组合：「服务」（接口地址、获取模型列表和自定义提示词收在它的「更多选项」里）、宿主让用户选数量时的「联想」，以及触屏宿主的测试工具。 */
export function AiSettingsPageSection({
  endpointValid = true,
  endpointHint,
  disabled,
  hidden,
  enabled,
  enabledDescription,
  provider,
  providerOptions,
  model,
  presetModels,
  endpoint,
  providerPreset,
  onEnabledChange,
  onProviderChange,
  onModelChange,
  onEndpointChange,
  credentialSection,
  desktopCredentialTest,
  modelCatalog,
  candidateLimitShown,
  candidateLimit,
  onCandidateLimitChange,
  promptId,
  promptCustom1,
  promptCustom2,
  promptCustom3,
  onPromptCustom1Change,
  onPromptCustom2Change,
  onPromptCustom3Change,
  testTools,
  trailing,
}: AiSettingsPageSectionProps) {
  const promptSlot = promptId || "custom_1";
  const slotPrompts = {
    custom_1: { name: "一", value: promptCustom1, onChange: onPromptCustom1Change },
    custom_2: { name: "二", value: promptCustom2, onChange: onPromptCustom2Change },
    custom_3: { name: "三", value: promptCustom3, onChange: onPromptCustom3Change },
  } as const;
  const slot = slotPrompts[promptSlot as keyof typeof slotPrompts] ?? slotPrompts.custom_1;
  return (
    <SettingsPageFieldset disabled={disabled} hidden={hidden} ariaLabel="AI 辅助">
      {/* 先把服务接通：开关、服务商、凭据、模型。接口地址一般随服务商预设，获取模型列表和自定义提示词也是接自定义服务或代理的同一批用户才用，一起收进「更多选项」；测试按钮放在组末，测的正是上面这些。 */}
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
        <ModelPickerRows
          title="模型"
          label="AI 模型"
          models={[...presetModels, ...(modelCatalog?.models ?? [])]}
          model={model}
          onChange={onModelChange}
        />
        <MoreOptions defaultOpen={!endpointValid}>
          <TextInputRow
            title="接口地址"
            description={endpoint ? endpointHint || undefined : undefined}
            label="AI 接口地址"
            type="url"
            value={endpoint}
            onChange={onEndpointChange}
          />
          {modelCatalog && (
            <>
              <ActionRow
                title="服务模型"
                description="从当前服务的模型目录读取，加进上面的模型列表；服务不支持时选「自定义…」自己填写。"
                action={modelCatalog.onFetch}
                disabled={modelCatalog.busy || !modelCatalog.origin}
                label={modelCatalog.busy ? "获取中…" : "获取模型列表"}
              />
              {modelCatalog.status && (
                <SettingsGroupNote role="status">{modelCatalog.status}</SettingsGroupNote>
              )}
            </>
          )}
          {/* 只编辑当前槽位（`prompt_id`）的提示词，存着的槽位和 `prompt_id` 都不动。填了就整个替换内置提示词，内置的那份规定了 JSON 输出格式，说明要讲清楚。 */}
          <SettingsManagerBlock>
            <SettingsTextareaField
              label="自定义提示词"
              description={`留空时使用内置的联想提示词。填写后会整个替换内置提示词，不是追加；内置提示词规定了只输出 JSON 格式的候选，自定义时也要这样要求，否则联想可能不出结果。${promptSlot === "custom_1" ? "" : `当前使用的是槽位${slot.name}。`}`}
              ariaLabel="AI 联想自定义提示词"
              value={slot.value}
              onChange={slot.onChange}
            />
          </SettingsManagerBlock>
        </MoreOptions>
        {desktopCredentialTest && <SettingsGroupBlock>{desktopCredentialTest}</SettingsGroupBlock>}
      </GroupList>
      {candidateLimitShown && (
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
      )}
      {testTools}
      {trailing}
    </SettingsPageFieldset>
  );
}
