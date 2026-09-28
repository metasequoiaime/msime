export interface AiCandidateLimitSectionProps {
  value: number;
  onChange: (value: number) => void;
}

function boundedCandidateLimit(value: string): number {
  return Math.max(1, Math.min(10, Number(value) || 3));
}

/** Candidate count control for the shared AI assistant settings. */
export function AiCandidateLimitSection({ value, onChange }: AiCandidateLimitSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">候选数量</span>
        <input
          aria-label="AI 候选数量"
          type="number"
          min="1"
          max="10"
          value={value}
          onChange={(event) => onChange(boundedCandidateLimit(event.target.value))}
        />
      </label>
    </div>
  );
}
