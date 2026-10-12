import { SwitchRow } from "./switch-row";

export interface ChineseEnglishSpacingSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 「中英文之间加空格」开关：输入页「输出」组里的一行，缺省为关，与共享偏好的默认值一致。 */
export function ChineseEnglishSpacingSection({
  value,
  onChange,
}: ChineseEnglishSpacingSectionProps) {
  return (
    <SwitchRow
      title="中英文之间加空格"
      description="一次上屏的文字里，汉字与英文字母、数字相邻处自动加一个空格，如「用Rust写」上屏为「用 Rust 写」。两次上屏之间不加；网址、日期等快捷模式和全角输入不受影响，译文和释义上屏、常用语、剪贴板和语音结果也不加"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
