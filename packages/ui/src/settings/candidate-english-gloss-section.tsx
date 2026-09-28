export interface CandidateEnglishGlossSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** Offline English gloss switch for hosts that expose candidate annotations. */
export function CandidateEnglishGlossSection({
  value,
  onChange,
}: CandidateEnglishGlossSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          显示英文释义
          <small>
            在候选词后面标出它的英文意思，中文候选给英文、英文候选给中文。释义来自随键盘打包的离线词库，不联网。
          </small>
        </span>
        <input
          aria-label="显示英文释义"
          className="toggle"
          type="checkbox"
          checked={value ?? false}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
