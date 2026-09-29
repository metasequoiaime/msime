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
}

/** Prompt preset, custom slot, editing, and restore controls for text polishing. */
export function PolishPromptSection({
  promptId,
  prompt,
  customPrompts = {},
  onSelectPrompt,
  onPromptChange,
  onRestore,
}: PolishPromptSectionProps) {
  const selectedSlot = normalizePolishSlot(promptId);
  const selectedDefault = isPolishCustomSlot(selectedSlot)
    ? (customPrompts[selectedSlot as keyof PolishCustomPromptValues] ?? "")
    : polishPresetPrompt(selectedSlot);
  const customSlot = isPolishCustomSlot(selectedSlot) ? selectedSlot : undefined;

  return (
    <>
      <label className="section-header">
        <span className="section-title">润色方案</span>
        <select
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
          <option value="custom_1">自定义一</option>
          <option value="custom_2">自定义二</option>
          <option value="custom_3">自定义三</option>
        </select>
      </label>
      <label className="section-header polish-prompt-row">
        <span className="section-title">
          润色提示词
          <small>
            {customSlot ? "这一段会保存到所选的自定义方案" : "内置方案的完整提示词，可以就地修改"}
          </small>
        </span>
        <textarea
          aria-label="润色提示词"
          value={prompt}
          onChange={(event) => onPromptChange(event.target.value, customSlot)}
        />
      </label>
      <button
        type="button"
        className="secondary"
        disabled={prompt === selectedDefault}
        onClick={() => onRestore(selectedDefault)}
      >
        恢复默认
      </button>
    </>
  );
}
