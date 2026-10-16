import {
  preeditControlState,
  type PreeditPreferences,
  type TsfPreeditStyle,
  type CandidatePreeditStyle,
} from "./preedit-preferences";
import { PreeditStyleRow } from "./preedit-style-row";
import { SwitchRow } from "./switch-row";

export type { TsfPreeditStyle, CandidatePreeditStyle } from "./preedit-preferences";

export interface PreeditSettingsPreferences extends PreeditPreferences {}

export interface PreeditSettingsSectionProps {
  preferences: PreeditSettingsPreferences;
  mobile: boolean;
  showShuangpinPreedit: boolean;
  inlinePreedit?: boolean;
  inlinePreeditBusy?: boolean;
  onChange: (patch: Partial<PreeditSettingsPreferences>) => void;
  onInlinePreeditChange?: (enabled: boolean) => void;
}

/** 实体键盘和触屏键盘宿主共用的预编辑显示控件：「候选窗口」页「预编辑」组的各行，候选窗口自己的预编辑在前，写进应用里的预编辑在最后。触屏键盘（iOS、Android、HarmonyOS 手机）固定把拼写以拼音分词写进输入框、候选栏不画它，所以只剩「双拼预编辑」，候选窗口预编辑和行内预编辑两行只给实体键盘宿主。 */
export function PreeditSettingsSection({
  preferences,
  mobile,
  showShuangpinPreedit,
  inlinePreedit,
  inlinePreeditBusy = false,
  onChange,
  onInlinePreeditChange,
}: PreeditSettingsSectionProps) {
  const state = preeditControlState(preferences);
  return (
    <>
      {!mobile && (
        <PreeditStyleRow
          title="候选窗口预编辑"
          mode="candidate"
          value={state.candidateStyle}
          onChange={(value) =>
            onChange({ candidate_preedit_style: value as CandidatePreeditStyle })
          }
        />
      )}
      {showShuangpinPreedit && (
        <PreeditStyleRow
          title="双拼预编辑"
          description="仅在双拼方案下生效；选择保留原始双拼按键，或显示展开后的拼音分词。"
          mode="shuangpin"
          value={state.shuangpinStyle}
          onChange={(value) => onChange({ shuangpin_preedit_uses_raw: value === "raw" })}
        />
      )}
      {!mobile &&
        (inlinePreedit !== undefined ? (
          <SwitchRow
            title="行内预编辑"
            description="把正在拼写的编码也写进输入框，像系统键盘那样带下划线显示。默认关闭；个别 App 显示不完整时可以关掉。"
            disabled={inlinePreeditBusy}
            checked={inlinePreedit}
            onChange={(checked) => onInlinePreeditChange?.(checked)}
          />
        ) : (
          <PreeditStyleRow
            title="行内预编辑"
            mode="inline"
            value={state.inlineStyle}
            onChange={(value) => onChange({ tsf_preedit_style: value as TsfPreeditStyle })}
          />
        ))}
    </>
  );
}
