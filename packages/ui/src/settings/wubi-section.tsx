import { SwitchRow } from "./switch-row";

export interface WubiPreferences {
  wubi_mixed_pinyin?: boolean;
  wubi_code_hint?: boolean;
  /** 缺省为开：本次之前它没有开关，唯一可能的实际行为就是开。 */
  wubi_auto_commit_unique?: boolean;
}

export interface WubiSectionProps {
  preferences: WubiPreferences;
  /** 文档里没有 `wubi_mixed_pinyin` 时混拼开关显示的值：本版本的默认值（`HostCapabilities.edition.wubi_mixed_pinyin_default`），缺省为关。 */
  mixedPinyinDefault?: boolean;
  onChange: (patch: Partial<WubiPreferences>) => void;
}

/** Shared Wubi fallback and completion controls for hosts that expose them: rows of the 方案 group while Wubi is in use. */
export function WubiSection({
  preferences,
  mixedPinyinDefault = false,
  onChange,
}: WubiSectionProps) {
  return (
    <>
      <SwitchRow
        title="五笔拼音混输"
        description="五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。"
        checked={preferences.wubi_mixed_pinyin ?? mixedPinyinDefault}
        onChange={(checked) => onChange({ wubi_mixed_pinyin: checked })}
      />
      <SwitchRow
        title="候选显示剩余编码"
        description="在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。"
        checked={preferences.wubi_code_hint ?? true}
        onChange={(checked) => onChange({ wubi_code_hint: checked })}
      />
      <SwitchRow
        title="五笔四码唯一候选自动上屏"
        description="五笔输入达到四码且只有一个候选时，自动提交该候选。关闭后这个词留在候选列表里，用空格或数字键选它。"
        checked={preferences.wubi_auto_commit_unique ?? true}
        onChange={(checked) => onChange({ wubi_auto_commit_unique: checked })}
      />
    </>
  );
}
