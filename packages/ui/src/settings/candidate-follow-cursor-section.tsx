export interface CandidateFollowCursorSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Candidate-window positioning switch for hosts that expose the setting. */
export function CandidateFollowCursorSection({
  value,
  onChange,
}: CandidateFollowCursorSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          候选窗口跟随光标
          <small>关闭后保持首次出现的位置，直到候选窗口消失。</small>
        </span>
        <input
          aria-label="候选窗口跟随光标"
          className="toggle"
          type="checkbox"
          checked={value ?? true}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
