export interface LearningDataSectionProps {
  disabled: boolean;
  onReset: () => void;
}

/** macOS-only controls for clearing the input method's learned data. */
export function LearningDataSection({ disabled, onReset }: LearningDataSectionProps) {
  return (
    <div className="section" role="region" aria-label="学习数据">
      <div className="section-header">
        <span className="section-title">
          学习数据
          <small>清除候选词频、用户词典和拼音学习记录；输入方案与其他设置不会改变。</small>
        </span>
        <button
          type="button"
          className="secondary danger-button"
          disabled={disabled}
          onClick={onReset}
        >
          清除全部学习数据
        </button>
      </div>
    </div>
  );
}
