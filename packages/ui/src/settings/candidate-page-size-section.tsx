import { Row, Select } from "../core/platform-controls";
import { offeredCandidatePageSizes } from "./candidate-page-size";

export interface CandidatePageSizeSectionProps {
  value: number;
  fixed: boolean;
  onChange: (value: number) => void;
}

/** Candidate page-size selector shared by hosts that expose a configurable candidate strip: one row of the 候选窗口 page's layout group. */
export function CandidatePageSizeSection({
  value,
  fixed,
  onChange,
}: CandidatePageSizeSectionProps) {
  if (fixed) return null;
  return (
    <Row title="每页候选项数量">
      <Select value={value} onChange={(event) => onChange(Number(event.target.value))}>
        {offeredCandidatePageSizes(value).map((size) => (
          <option key={size} value={size}>
            {size}
          </option>
        ))}
      </Select>
    </Row>
  );
}
