export interface CloudCandidatesSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Online candidate lookup switch shared by input settings hosts. */
export function CloudCandidatesSection({ value, onChange }: CloudCandidatesSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          云候选<small>向在线服务请求额外候选</small>
        </span>
        <input
          aria-label="云候选"
          className="toggle"
          type="checkbox"
          checked={value ?? true}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
