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
          onChange={(event) => onChange(Math.max(1, Math.min(10, Number(event.target.value) || 3)))}
        />
      </label>
    </div>
  );
}
