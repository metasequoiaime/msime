export interface LearningSectionProps {
  value: boolean;
  onChange: (value: boolean) => void;
}

/** Learning preference switch shared by hosts that expose the input settings page. */
export function LearningSection({ value, onChange }: LearningSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          学习选词习惯<small>根据选词调整候选顺序</small>
        </span>
        <input
          aria-label="学习选词习惯"
          className="toggle"
          type="checkbox"
          checked={value}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
