import { SwitchRow } from "./switch-row";

export interface CandidatePageNumberSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** The 显示页码 switch for hosts that draw a page number (`HostCapabilities::candidate_page_number`): one row of the 候选窗口 page's 布局 group. */
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
