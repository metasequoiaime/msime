import { SwitchRow } from "./switch-row";

export interface EnglishSuggestionsSectionProps {
  value?: boolean;
  disabled?: boolean;
  onChange: (value: boolean) => void;
}

/** English completion switch for hosts that expose the candidate suggestion feature: one row of the 多语言候选 group on the 标点与翻译 page. */
export function EnglishSuggestionsSection({
  value,
  disabled = false,
  onChange,
}: EnglishSuggestionsSectionProps) {
  return (
    <SwitchRow
      title="英文建议"
      description="英文 26 键直接输入时，在候选栏显示当前单词的补全建议；关闭后仍可正常输入英文。"
      disabled={disabled}
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
