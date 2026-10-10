import { offeredCandidatePageSizes } from "./candidate-page-size";
import { SliderRow } from "./slider-row";

export interface CandidatePageSizeSectionProps {
  value: number;
  fixed: boolean;
  /** 宿主能排的最大每页候选数（`HostCapabilities.max_candidate_page_size`），缺省为 9。 */
  max?: number;
  onChange: (value: number) => void;
}

/** Candidate page-size slider shared by hosts that expose a configurable candidate strip: one row of the 候选窗口 page's layout group. A ticked slider with the value after it, as the reference window draws this row; a stored size below the offered range widens the track down to it rather than being rewritten. */
export function CandidatePageSizeSection({
  value,
  fixed,
  max,
  onChange,
}: CandidatePageSizeSectionProps) {
  if (fixed) return null;
  const sizes = offeredCandidatePageSizes(value, max);
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
