import { CustomPromptSlotOptions } from "./custom-prompt-slot-options";
import { SelectSettingField } from "./select-setting-field";
import { SettingTextarea } from "./setting-textarea";

export function AiPromptSettingsSection({
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
}: {
  promptId?: string;
  prompt?: string;
  promptCustom1: string;
  promptCustom2: string;
  promptCustom3: string;
  fallbackPrompt: string;
  onPromptIdChange: (value: string) => void;
  onPromptChange: (value: string) => void;
  onPromptCustom1Change: (value: string) => void;
  onPromptCustom2Change: (value: string) => void;
  onPromptCustom3Change: (value: string) => void;
}) {
  return (
    <>
      <div className="section">
        <SelectSettingField
          label="AI 联想提示词方案"
          inputLabel="AI 联想提示词方案"
          description="使用选中的独立槽位；槽位留空时使用兼容提示词"
          value={promptId === "custom" ? "custom_1" : promptId || "custom_1"}
          onChange={onPromptIdChange}
        >
          <CustomPromptSlotOptions />
        </SelectSettingField>
      </div>
      <SettingTextarea
        label="兼容提示词"
        description="旧版提示词，所选自定义槽位留空时使用"
        ariaLabel="AI 润色提示词"
        placeholder="留空时使用内置的联想提示词"
        value={prompt ?? fallbackPrompt}
        onChange={onPromptChange}
      />
      <SettingTextarea
        label="自定义提示词一"
        description="发送给 AI 联想服务的额外提示词"
        ariaLabel="自定义提示词一"
        value={promptCustom1}
        onChange={onPromptCustom1Change}
      />
      <SettingTextarea
        label="自定义提示词二"
        ariaLabel="自定义提示词二"
        value={promptCustom2}
        onChange={onPromptCustom2Change}
      />
      <SettingTextarea
        label="自定义提示词三"
        ariaLabel="自定义提示词三"
        value={promptCustom3}
        onChange={onPromptCustom3Change}
      />
    </>
  );
}
