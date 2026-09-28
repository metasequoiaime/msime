export interface WubiPreferences {
  wubi_mixed_pinyin?: boolean;
  wubi_code_hint?: boolean;
}

export interface WubiSectionProps {
  preferences: WubiPreferences;
  autoCommitUnique?: boolean;
  onChange: (patch: Partial<WubiPreferences>) => void;
  onAutoCommitUniqueChange?: (value: boolean) => void;
}

/** Shared Wubi fallback and completion controls for hosts that expose them. */
export function WubiSection({
  preferences,
  autoCommitUnique,
  onChange,
  onAutoCommitUniqueChange,
}: WubiSectionProps) {
  return (
    <div className="section" role="group" aria-label="五笔">
      <label className="section-header">
        <span className="section-title">
          编码打不出时用拼音候选
          <small>五笔词库无法回答当前编码时，用同一串字母查询全拼；词库能回答时不影响。</small>
        </span>
        <input
          aria-label="编码打不出时用拼音候选"
          className="toggle"
          type="checkbox"
          checked={preferences.wubi_mixed_pinyin ?? false}
          onChange={(event) => onChange({ wubi_mixed_pinyin: event.target.checked })}
        />
      </label>
      <label className="section-header">
        <span className="section-title">
          候选显示剩余编码
          <small>在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。</small>
        </span>
        <input
          aria-label="候选显示剩余编码"
          className="toggle"
          type="checkbox"
          checked={preferences.wubi_code_hint ?? true}
          onChange={(event) => onChange({ wubi_code_hint: event.target.checked })}
        />
      </label>
      {autoCommitUnique !== undefined && (
        <label className="section-header">
          <span className="section-title">
            五笔四码唯一候选自动上屏
            <small>五笔输入达到四码且只有一个候选时，自动提交该候选。</small>
          </span>
          <input
            aria-label="五笔四码唯一候选自动上屏"
            className="toggle"
            type="checkbox"
            checked={autoCommitUnique}
            onChange={(event) => onAutoCommitUniqueChange?.(event.target.checked)}
          />
        </label>
      )}
    </div>
  );
}
