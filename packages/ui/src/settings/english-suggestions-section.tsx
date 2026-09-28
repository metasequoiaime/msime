export interface EnglishSuggestionsSectionProps {
  value?: boolean;
  onChange: (value: boolean) => void;
}

/** English completion switch for hosts that expose the candidate suggestion feature. */
export function EnglishSuggestionsSection({ value, onChange }: EnglishSuggestionsSectionProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          英文建议
          <small>
            英文 26 键直接输入时，在候选栏显示当前单词的补全建议；关闭后仍可正常输入英文。
          </small>
        </span>
        <input
          aria-label="英文建议"
          className="toggle"
          type="checkbox"
          checked={value ?? true}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    </div>
  );
}
