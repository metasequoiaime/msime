export type CandidateLayout = "horizontal" | "vertical";

export interface CandidateLayoutSectionProps {
  value?: CandidateLayout;
  fixed: boolean;
  onChange: (value: CandidateLayout) => void;
}

/** Candidate orientation selector for hosts whose candidate panel exposes a choice. */
export function CandidateLayoutSection({ value, fixed, onChange }: CandidateLayoutSectionProps) {
  if (fixed) return null;
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">候选项排列方式</span>
        <select
          aria-label="候选项排列方式"
          value={value ?? "vertical"}
          onChange={(event) => onChange(event.target.value as CandidateLayout)}
        >
          <option value="horizontal">横向</option>
          <option value="vertical">纵向</option>
        </select>
      </label>
    </div>
  );
}
