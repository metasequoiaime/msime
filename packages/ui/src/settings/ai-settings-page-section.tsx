import { SettingsGroupNote } from "./settings-group-note";
import type { ReactNode } from "react";
import { clamp } from "../core/number";
import { GroupList, MoreOptions } from "../core/platform-controls";
import { ModelPickerRows } from "./model-picker-rows";
import { SettingsTextareaField } from "./settings-textarea-field";
import { SwitchRow } from "./switch-row";
import { SelectRow } from "./select-row";
import { AiProviderSelect } from "./ai-provider-select";
import { TextInputRow } from "./text-input-row";
import { ActionRow } from "./action-row";
import { SettingsPageFieldset } from "./settings-page-fieldset";
import { SettingsGroupBlock } from "./settings-group-block";
import { SettingsManagerBlock } from "./settings-manager-block";
import * as settings from "./settings-style";
import type { AiProviderOption } from "./ai-provider-options";

export type { AiProviderOption } from "./ai-provider-options";

export interface AiSettingsPageSectionProps {
  /** AI 服务的接口地址是否有效，用于地址栏的无效状态；凭据仍由 origin 校验控制。 */
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
  /** DeepSeek 思考强度（`ai_assistant.thinking`），选中 DeepSeek 时显示档位下拉。 */
  thinking?: string;
  onThinkingChange?: (thinking: string) => void;
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
  /** 「网络」一组，放在测试工具之前：代理设置对云候选、翻译、AI、语音和下载都生效。 */
  networkSection?: ReactNode;
  /** 最后一组之后的内容：设置页在这里放进入「AI 对话」的入口。 */
  trailing: ReactNode;
}

/** 共享 AI 辅助设置的页面组合：按依赖顺序配置服务，自定义提示词单独折叠。 */
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
  thinking,
  onThinkingChange,
  promptId,
  promptCustom1,
  promptCustom2,
  promptCustom3,
  onPromptCustom1Change,
  onPromptCustom2Change,
  onPromptCustom3Change,
  testTools,
  networkSection,
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
    <SettingsPageFieldset
      disabled={disabled}
      hidden={hidden}
      ariaLabel="AI 辅助"
      className="ai-settings"
    >
      {/* 接口地址是凭据与模型目录的前提，始终放在它们上方；校验变化不搬动输入节点。 */}
      <GroupList title="服务">
        <SwitchRow
          title="启用 AI 辅助"
          description={enabledDescription}
          aria-label="启用 AI 辅助"
          checked={enabled}
          onChange={onEnabledChange}
        />
        <AiProviderSelect
          options={providerOptions}
          value={provider}
          disabled={disabled}
          hidden={hidden}
          onChange={onProviderChange}
        />
        <TextInputRow
          title="接口地址"
          detail={endpoint ? endpointHint || undefined : "请填写服务提供商给出的完整接口地址"}
          label="AI 接口地址"
          type="url"
          placeholder="https://example.com/v1/chat/completions"
          aria-invalid={endpoint && !endpointValid ? true : undefined}
          value={endpoint}
          onChange={onEndpointChange}
        />
        {credentialSection}
        {providerPreset}
        <ModelPickerRows
          title="模型"
          label="AI 模型"
          models={[...presetModels, ...(modelCatalog?.models ?? [])]}
          model={model}
          onChange={onModelChange}
        />
        {onThinkingChange && (
          <SelectRow
            title="思考强度"
            description="开启后候选更准，但要多等几秒到几十秒。"
            hidden={provider !== "deepseek"}
            value={thinking ?? "disabled"}
            aria-label="思考强度"
            onChange={(event) => onThinkingChange(event.currentTarget.value)}
          >
            <option value="disabled">关闭</option>
            <option value="low">低</option>
            <option value="high">高</option>
            <option value="max">最高</option>
          </SelectRow>
        )}
        {modelCatalog && (
          <>
            <ActionRow
              title="服务模型"
              description="可从服务获取模型，也可直接填写模型名称。"
              action={modelCatalog.onFetch}
              disabled={modelCatalog.busy || !modelCatalog.origin}
              label={modelCatalog.busy ? "获取中…" : "获取模型列表"}
            />
            {modelCatalog.status && (
              <SettingsGroupNote role="status">{modelCatalog.status}</SettingsGroupNote>
            )}
          </>
        )}
        <MoreOptions title="自定义提示词">
          {/* 只编辑当前槽位（`prompt_id`）的提示词，存着的槽位和 `prompt_id` 都不动。填了就整个替换内置提示词，内置的那份规定了 JSON 输出格式，说明要讲清楚。 */}
          <SettingsManagerBlock>
            <SettingsTextareaField
              label="提示词内容"
              description={
                <span className="block mt-1">
                  留空使用内置提示词；填写后完整替换，请要求只输出 JSON 格式的候选。
                  {promptSlot === "custom_1" ? "" : `当前使用槽位${slot.name}。`}
                </span>
              }
              rows={4}
              className={`${settings.fieldInput} min-h-24 w-full resize-y leading-relaxed`}
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
      {networkSection}
      {testTools}
      {trailing}
    </SettingsPageFieldset>
  );
}
