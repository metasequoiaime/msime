import { SwitchRow } from "./switch-row";
import {
  wubiControlState,
  type WubiPreferences as SharedWubiPreferences,
} from "./wubi-preferences";

export interface WubiPreferences extends SharedWubiPreferences {}

export interface WubiSectionProps {
  preferences: WubiPreferences;
  /** 文档里没有 `wubi_mixed_pinyin` 时混拼开关显示的值：本版本的默认值（`HostCapabilities.edition.wubi_mixed_pinyin_default`），缺省为关。 */
  mixedPinyinDefault?: boolean;
  onChange: (patch: Partial<WubiPreferences>) => void;
}

/** 使用五笔时显示在「方案」组里的混输、编码提示和自动上屏控件。 */
export function WubiSection({
  preferences,
  mixedPinyinDefault = false,
  onChange,
}: WubiSectionProps) {
  const state = wubiControlState(preferences, mixedPinyinDefault);
  return (
    <>
      <SwitchRow
        title="五笔拼音混输"
        description="五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。"
        checked={state.wubi_mixed_pinyin}
        onChange={(checked) => onChange({ wubi_mixed_pinyin: checked })}
      />
      <SwitchRow
        title="候选显示剩余编码"
        description="在候选后面标出还要再打哪几个字母才能单独打出它。已经打完整码的候选不标。"
        checked={state.wubi_code_hint}
        onChange={(checked) => onChange({ wubi_code_hint: checked })}
      />
      <SwitchRow
        title="五笔四码唯一候选自动上屏"
        description="五笔输入达到四码且只有一个候选时，自动提交该候选。关闭后这个词留在候选列表里，用空格或数字键选它。"
        checked={state.wubi_auto_commit_unique}
        onChange={(checked) => onChange({ wubi_auto_commit_unique: checked })}
      />
    </>
  );
}
