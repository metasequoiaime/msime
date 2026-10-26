import { SwitchRow } from "./switch-row";

export interface CaretEditSuffixSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 「移动光标后编辑光标后的部分」开关：输入页「候选与联想」组「更多选项」里的一行，排在快捷短语混输之前。缺省为关，与共享偏好的默认值一致；只在声明 `HostCapabilities.caret_edit_suffix` 的桌面宿主上列出。 */
export function CaretEditSuffixSection({ value, onChange }: CaretEditSuffixSectionProps) {
  return (
    <SwitchRow
      title="移动光标后编辑光标后的部分"
      description="组字光标移到拼音中间时，光标前的部分固定为当前转换，候选改为编辑光标后的部分；首选是整句"
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
