import { SwitchRow } from "./switch-row";

export interface CandidatePageNumberSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 「显示页码」开关，供会绘制页码的宿主使用（`HostCapabilities::candidate_page_number`）：「候选窗口」页「布局」组中的一行。 */
export function CandidatePageNumberSection({ value, onChange }: CandidatePageNumberSectionProps) {
  return (
    <SwitchRow
      title="显示页码"
      description="显示候选列表的当前页与总页数；关闭后仍可正常翻页。"
      checked={value !== false}
      onChange={onChange}
    />
  );
}
