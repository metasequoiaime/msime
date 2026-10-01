import type { ReactNode } from "react";
import { SelectRow } from "./select-row";
import * as settings from "./settings-style";
import { CustomPromptSlotOptions } from "./custom-prompt-slot-options";
import {
  POLISH_CUSTOM_IDS,
  POLISH_PRESET_IDS,
  POLISH_PRESET_NAMES,
  isPolishCustomSlot,
  normalizePolishSlot,
  polishPresetPrompt,
} from "../voice/polish-presets";

export type PolishCustomPromptValues = Partial<
  Record<(typeof POLISH_CUSTOM_IDS)[number], string | undefined>
>;

export interface PolishPromptSectionProps {
  promptId?: string;
  prompt: string;
  customPrompts?: PolishCustomPromptValues;
  onSelectPrompt: (promptId: string, prompt: string) => void;
  onPromptChange: (prompt: string, customSlot?: string) => void;
  onRestore: (prompt: string) => void;
  /** Further buttons beside 恢复默认, such as the host's credential test. */
  actions?: ReactNode;
}

/** Prompt preset, custom slot, editing, and restore controls for text polishing. */
export function PolishPromptSection({
  promptId,
  prompt,
  customPrompts = {},
  onSelectPrompt,
  onPromptChange,
  onRestore,
  actions,
}: PolishPromptSectionProps) {
  const selectedSlot = normalizePolishSlot(promptId);
  const selectedDefault = isPolishCustomSlot(selectedSlot)
    ? (customPrompts[selectedSlot as keyof PolishCustomPromptValues] ?? "")
    : polishPresetPrompt(selectedSlot);
  const customSlot = isPolishCustomSlot(selectedSlot) ? selectedSlot : undefined;

  return (
    <>
      <SelectRow
        title="润色方案"
        aria-label="润色方案"
        value={selectedSlot}
        onChange={(event) => {
          const nextSlot = normalizePolishSlot(event.target.value);
          const nextDefault = isPolishCustomSlot(nextSlot)
            ? (customPrompts[nextSlot as keyof PolishCustomPromptValues] ?? "")
            : polishPresetPrompt(nextSlot);
          onSelectPrompt(nextSlot, nextDefault);
        }}
      >
        {POLISH_PRESET_IDS.map((id) => (
          <option key={id} value={id}>
            {POLISH_PRESET_NAMES[id]}
          </option>
        ))}
        <CustomPromptSlotOptions />
      </SelectRow>
      <div className={settings.managerBlock}>
        <label className={settings.field}>
          <span>
            <span data-row-title="">润色提示词</span>{" "}
            {customSlot ? "这一段会保存到所选的自定义方案" : "内置方案的完整提示词，可以就地修改"}
          </span>
          <textarea
            aria-label="润色提示词"
            className={settings.promptInput}
            value={prompt}
            onChange={(event) => onPromptChange(event.target.value, customSlot)}
          />
        </label>
        <div className={settings.managerActions}>
          <button
            type="button"
            className="secondary"
            disabled={prompt === selectedDefault}
            onClick={() => onRestore(selectedDefault)}
          >
            恢复默认
          </button>
          {actions}
        </div>
      </div>
    </>
  );
}
