export interface CandidatePageSizeSectionProps {
  value: number;
  fixed: boolean;
  onChange: (value: number) => void;
}

const candidatePageSizes = Array.from({ length: 7 }, (_, index) => index + 3);

function offeredCandidatePageSizes(current: number): number[] {
  return candidatePageSizes.includes(current)
    ? candidatePageSizes
    : [current, ...candidatePageSizes];
}

/** Candidate page-size selector shared by hosts that expose a configurable candidate strip. */
export function CandidatePageSizeSection({
  value,
  fixed,
  onChange,
}: CandidatePageSizeSectionProps) {
  if (fixed) return null;
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">每页候选项数量</span>
        <select
          aria-label="每页候选项数量"
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
        >
          {offeredCandidatePageSizes(value).map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}
