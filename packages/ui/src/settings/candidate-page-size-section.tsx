import { offeredCandidatePageSizes } from "./candidate-page-size";
import { SliderRow } from "./slider-row";

export interface CandidatePageSizeSectionProps {
  value: number;
  fixed: boolean;
  onChange: (value: number) => void;
}

/** Candidate page-size slider shared by hosts that expose a configurable candidate strip: one row of the 候选窗口 page's layout group. A ticked slider with the value after it, as the reference window draws this row; a stored size below the offered range widens the track down to it rather than being rewritten. */
export function CandidatePageSizeSection({
  value,
  fixed,
  onChange,
}: CandidatePageSizeSectionProps) {
  if (fixed) return null;
  const sizes = offeredCandidatePageSizes(value);
  return (
    <SliderRow
      title="每页候选项数量"
      ticks
      min={Math.min(...sizes)}
      max={Math.max(...sizes)}
      value={value}
      displayValue={value}
      onChange={onChange}
    />
  );
}
