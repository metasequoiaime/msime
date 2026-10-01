import { SwitchRow } from "./switch-row";

export interface EnglishSuggestionsSectionProps {
  value?: boolean;
  disabled?: boolean;
  onChange: (value: boolean) => void;
}

/** 英文补全开关，供提供候选建议功能的宿主使用：「标点与翻译」页「多语言与释义」组中的一行。 */
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
