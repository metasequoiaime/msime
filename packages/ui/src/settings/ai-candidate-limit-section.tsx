import { clamp } from "../core/number";

export function AiCandidateLimitSection({
  value,
  onChange,
}: {
  value: number;
  onChange: (value: number) => void;
}) {
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
          onChange={(event) => onChange(clamp(Number(event.target.value) || 3, 1, 10))}
        />
      </label>
    </div>
  );
}
