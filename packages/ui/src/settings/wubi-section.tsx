import { SwitchRow } from "./switch-row";

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

/** Shared Wubi fallback and completion controls for hosts that expose them: rows of the 方案 group while Wubi is in use. */
export function WubiSection({
  preferences,
  autoCommitUnique,
  onChange,
  onAutoCommitUniqueChange,
}: WubiSectionProps) {
  return (
    <>
      <SwitchRow
        title="编码打不出时用拼音候选"
        description="五笔词库无法回答当前编码时，用同一串字母查询全拼；词库能回答时不影响。"
        checked={preferences.wubi_mixed_pinyin ?? false}
        onChange={(checked) => onChange({ wubi_mixed_pinyin: checked })}
      />
      <SwitchRow
        title="候选显示剩余编码"
        description="在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。"
        checked={preferences.wubi_code_hint ?? true}
        onChange={(checked) => onChange({ wubi_code_hint: checked })}
      />
      {autoCommitUnique !== undefined && (
        <SwitchRow
          title="五笔四码唯一候选自动上屏"
          description="五笔输入达到四码且只有一个候选时，自动提交该候选。"
          checked={autoCommitUnique}
          onChange={(checked) => onAutoCommitUniqueChange?.(checked)}
        />
      )}
    </>
  );
}
