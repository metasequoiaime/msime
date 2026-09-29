import { Row, Segmented } from "../core/platform-controls";

export type CandidateLayout = "horizontal" | "vertical";

export interface CandidateLayoutSectionProps {
  value?: CandidateLayout;
  fixed: boolean;
  onChange: (value: CandidateLayout) => void;
}

const candidateLayoutOptions = [
  { value: "horizontal", label: "横向" },
  { value: "vertical", label: "纵向" },
] as const satisfies readonly { value: CandidateLayout; label: string }[];

/** Candidate orientation selector for hosts whose candidate panel exposes a choice: one row of the 候选窗口 page's layout group. */
export function CandidateLayoutSection({ value, fixed, onChange }: CandidateLayoutSectionProps) {
  if (fixed) return null;
  return (
    <Row title="候选项排列方式">
      <Segmented options={candidateLayoutOptions} value={value ?? "vertical"} onChange={onChange} />
    </Row>
  );
}
