import type { ReactNode } from "react";
import { SwitchRow } from "./switch-row";

export interface EnglishSuggestionsSectionProps {
  value?: boolean;
  disabled?: boolean;
  /** 行标题。默认为「英文建议」；鸿蒙手机的「表达」页与 Android 一样称其为「英文联想」。 */
  title?: string;
  /** 行副标题。默认为下面的说明；`null` 时不画副标题，与鸿蒙手机设计稿一致。 */
  description?: ReactNode;
  onChange: (value: boolean) => void;
}

/** 英文补全开关，供提供候选建议功能的宿主使用：「标点与翻译」页「多语言与释义」组中的一行。 */
export function EnglishSuggestionsSection({
  value,
  disabled = false,
  title = "英文建议",
  description = "英文 26 键直接输入时，在候选栏显示当前单词的补全建议；关闭后仍可正常输入英文。",
  onChange,
}: EnglishSuggestionsSectionProps) {
  return (
    <SwitchRow
      title={title}
      description={description}
      disabled={disabled}
      checked={value ?? true}
      onChange={onChange}
    />
  );
}
