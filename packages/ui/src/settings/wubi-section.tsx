import { useState } from "react";
import { ActionRow } from "./action-row";
import { TextInputRow } from "./text-input-row";
import {
  defaultWubiCandidateSelection,
  validWubiCandidateKeys,
  type WubiCandidateSelectionPreferences,
} from "./wubi-preferences";
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

/** 使用五笔时显示在「方案」组里的混输、编码提示、z 通配、顶字和自动上屏控件。 */
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
      <WubiCandidateSelectionSection
        key={JSON.stringify(preferences.wubi_candidate_selection ?? defaultWubiCandidateSelection)}
        settings={preferences.wubi_candidate_selection ?? defaultWubiCandidateSelection}
        onChange={(wubi_candidate_selection) => onChange({ wubi_candidate_selection })}
      />
      <SwitchRow
        title="z 键通配"
        description="开启后 z 可匹配码表中该位置的任意字母（含首位），常用于首码不确定时起头；同时停用 z 引导的临时拼音反查。"
        checked={state.wubi_z_wildcard}
        onChange={(checked) => onChange({ wubi_z_wildcard: checked })}
      />
      <SwitchRow
        title="第五码将首选上屏"
        description="完整四码后再敲字母时上屏首选，并把该字母留作下一组开头；关闭只在拼音混输开启时生效，这个字母并入混输串。混输关闭时仍按顶字上屏，不会丢字。"
        checked={state.wubi_fifth_code_top_commit}
        onChange={(checked) => onChange({ wubi_fifth_code_top_commit: checked })}
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

/** 两个字符一起保存，允许交换键位，编辑中的空值不会进入共享文档。 */
function WubiCandidateSelectionSection({
  settings,
  onChange,
}: {
  settings: WubiCandidateSelectionPreferences;
  onChange: (settings: WubiCandidateSelectionPreferences) => void;
}) {
  const [second, setSecond] = useState(settings.second_key);
  const [third, setThird] = useState(settings.third_key);
  const valid = validWubiCandidateKeys(second, third);
  return (
    <>
      <SwitchRow
        title="五笔快捷选词"
        description="组字时用两个标点选当前页第 2、3 候选；候选不足时仍输入标点。"
        checked={settings.enabled}
        onChange={(enabled) => onChange({ ...settings, enabled })}
      />
      {settings.enabled && (
        <>
          <TextInputRow
            title="第 2 候选键"
            label="五笔第 2 候选键"
            value={second}
            maxLength={1}
            onChange={setSecond}
          />
          <TextInputRow
            title="第 3 候选键"
            label="五笔第 3 候选键"
            value={third}
            maxLength={1}
            onChange={setThird}
          />
          <ActionRow
            title="自定义选词键"
            label="保存选词键"
            description="使用两个不同的英文标点；请选择未用于翻页或以词定字的键。"
            detail={valid ? undefined : "请输入两个不同的单个英文标点。"}
            disabled={!valid || (second === settings.second_key && third === settings.third_key)}
            action={() => onChange({ ...settings, second_key: second, third_key: third })}
          />
        </>
      )}
    </>
  );
}
