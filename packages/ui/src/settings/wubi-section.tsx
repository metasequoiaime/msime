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
      <SettingToggle
        label="编码打不出时用拼音候选"
        description="五笔词库无法回答当前编码时，用同一串字母查询全拼；词库能回答时不影响。"
        ariaLabel="编码打不出时用拼音候选"
        checked={preferences.wubi_mixed_pinyin ?? false}
        compact
        onChange={(enabled) => onChange({ wubi_mixed_pinyin: enabled })}
      />
      <SettingToggle
        label="候选显示剩余编码"
        description="在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。"
        ariaLabel="候选显示剩余编码"
        checked={preferences.wubi_code_hint ?? true}
        compact
        onChange={(enabled) => onChange({ wubi_code_hint: enabled })}
      />
      {autoCommitUnique !== undefined && (
        <SettingToggle
          label="五笔四码唯一候选自动上屏"
          description="五笔输入达到四码且只有一个候选时，自动提交该候选。"
          ariaLabel="五笔四码唯一候选自动上屏"
          checked={autoCommitUnique}
          compact
          onChange={(enabled) => onAutoCommitUniqueChange?.(enabled)}
        />
      )}
    </div>
  );
}
import { SettingToggle } from "./setting-toggle";
