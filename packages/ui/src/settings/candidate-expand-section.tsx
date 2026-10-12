import { SwitchRow } from "./switch-row";

export interface CandidateExpandSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** 「展开候选」开关，供按 `expand_candidates` 画多行候选网格的宿主使用（`HostCapabilities::expand_candidates`，目前只有 macOS）：「候选窗口」页「布局」组中的一行。默认关闭，所以缺值读成关。 */
export function CandidateExpandSection({ value, onChange }: CandidateExpandSectionProps) {
  return (
    <SwitchRow
      title="展开候选"
      description="横排候选窗按 ↓ 或点展开按钮，把后面几页候选排成多行一起显示；竖排候选窗不受影响。"
      checked={value === true}
      onChange={onChange}
    />
  );
}
