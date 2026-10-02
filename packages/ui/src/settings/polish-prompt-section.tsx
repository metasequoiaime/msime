import type { ReactNode } from "react";
import { SelectRow } from "./select-row";
import { SettingsTextareaField } from "./settings-textarea-field";
import * as settings from "./settings-style";
import { CustomPromptSlotOptions } from "./custom-prompt-slot-options";
import {
  POLISH_CUSTOM_IDS,
  POLISH_PRESET_IDS,
  POLISH_PRESET_NAMES,
  isPolishCustomSlot,
  polishPresetPrompt,
} from "../voice/polish-presets";

export type PolishCustomSlot = (typeof POLISH_CUSTOM_IDS)[number];
export type PolishCustomPromptValues = Partial<Record<PolishCustomSlot, string | undefined>>;

export interface PolishPromptSectionProps {
  promptId?: string;
  customPrompts?: PolishCustomPromptValues;
  onSelectPrompt: (promptId: string) => void;
  onCustomPromptChange: (slot: PolishCustomSlot, prompt: string) => void;
  /** Further buttons below the prompt, such as the host's credential test. */
  actions?: ReactNode;
}

/** Prompt preset and custom slot controls for text polishing. A preset shows its built-in prompt read-only; a custom slot is edited in place, and an empty slot sends the built-in 精炼整理 prompt. */
export function PolishPromptSection({
  promptId,
  customPrompts = {},
  onSelectPrompt,
  onCustomPromptChange,
  actions,
}: PolishPromptSectionProps) {
  const selectedSlot = promptId || "cleanup";
  const customSlot = isPolishCustomSlot(selectedSlot)
    ? (selectedSlot as PolishCustomSlot)
    : undefined;

  return (
    <>
      <SelectRow
        title="润色方案"
        aria-label="润色方案"
        value={selectedSlot}
        onChange={(event) => onSelectPrompt(event.target.value)}
      >
        {POLISH_PRESET_IDS.map((id) => (
          <option key={id} value={id}>
            {POLISH_PRESET_NAMES[id]}
          </option>
        ))}
        <CustomPromptSlotOptions />
      </SelectRow>
      <div className={settings.managerBlock}>
        {customSlot ? (
          <SettingsTextareaField
            label="润色提示词"
            ariaLabel="润色提示词"
            description="这一段会保存到所选的自定义方案；留空时使用内置的「精炼整理」提示词"
            value={customPrompts[customSlot] ?? ""}
            onChange={(value) => onCustomPromptChange(customSlot, value)}
          />
        ) : (
          <SettingsTextareaField
            label="润色提示词"
            ariaLabel="润色提示词"
            description="内置方案的完整提示词；要改写请选择自定义方案"
            value={polishPresetPrompt(selectedSlot)}
            onChange={() => {}}
            readOnly
          />
        )}
        {actions && <div className={settings.managerActions}>{actions}</div>}
      </div>
    </>
  );
}
