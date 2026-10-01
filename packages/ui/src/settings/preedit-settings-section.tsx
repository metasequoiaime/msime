import { Row, Switch } from "../core/platform-controls";
import { PreeditStyleSelect } from "./preedit-style-select";

export type TsfPreeditStyle = "raw" | "pinyin" | "empty";
export type CandidatePreeditStyle = "pinyin" | "empty";

export interface PreeditSettingsPreferences {
  shuangpin_preedit_uses_raw?: boolean;
  tsf_preedit_style?: TsfPreeditStyle;
  candidate_preedit_style?: CandidatePreeditStyle;
}

export interface PreeditSettingsSectionProps {
  preferences: PreeditSettingsPreferences;
  mobile: boolean;
  showShuangpinPreedit: boolean;
  inlinePreedit?: boolean;
  inlinePreeditBusy: boolean;
  onChange: (patch: Partial<PreeditSettingsPreferences>) => void;
  onInlinePreeditChange?: (enabled: boolean) => void;
}

/** 实体键盘和触屏键盘宿主共用的预编辑显示控件：「候选窗口」页「预编辑」组的各行，候选窗口自己的预编辑在前，写进应用里的预编辑在最后。 */
export function PreeditSettingsSection({
  preferences,
  mobile,
  showShuangpinPreedit,
  inlinePreedit,
  inlinePreeditBusy,
  onChange,
  onInlinePreeditChange,
}: PreeditSettingsSectionProps) {
  return (
    <>
      <Row title={mobile ? "候选栏预编辑" : "候选窗口预编辑"}>
        <PreeditStyleSelect
          mode="candidate"
          value={preferences.candidate_preedit_style ?? "pinyin"}
          onChange={(value) =>
            onChange({ candidate_preedit_style: value as CandidatePreeditStyle })
          }
        />
      </Row>
      {showShuangpinPreedit && (
        <Row
          title="双拼预编辑"
          description="仅在双拼方案下生效；选择保留原始双拼按键，或显示展开后的拼音分词。"
        >
          <PreeditStyleSelect
            mode="shuangpin"
            value={preferences.shuangpin_preedit_uses_raw === false ? "pinyin" : "raw"}
            onChange={(value) => onChange({ shuangpin_preedit_uses_raw: value === "raw" })}
          />
        </Row>
      )}
      {inlinePreedit !== undefined ? (
        <Row
          title="行内预编辑"
          description="把正在拼写的编码也写进输入框，像系统键盘那样带下划线显示。默认关闭；个别 App 显示不完整时可以关掉。"
        >
          <Switch
            disabled={inlinePreeditBusy}
            checked={inlinePreedit}
            onChange={(checked) => onInlinePreeditChange?.(checked)}
          />
        </Row>
      ) : (
        <Row title="行内预编辑">
          <PreeditStyleSelect
            mode="inline"
            value={preferences.tsf_preedit_style ?? "raw"}
            onChange={(value) => onChange({ tsf_preedit_style: value as TsfPreeditStyle })}
          />
        </Row>
      )}
    </>
  );
}
