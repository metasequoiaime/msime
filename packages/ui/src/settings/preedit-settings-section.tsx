import { Row, Switch } from "../core/platform-controls";
import { PreeditStyleSelect } from "./preedit-style-select";
import { SwitchRow } from "./switch-row";

export type TsfPreeditStyle = "raw" | "pinyin" | "empty";
export type CandidatePreeditStyle = "pinyin" | "empty";

export interface PreeditSettingsPreferences {
  shuangpin_preedit_uses_raw?: boolean;
  tsf_preedit_style?: TsfPreeditStyle;
  candidate_preedit_style?: CandidatePreeditStyle;
  show_candidate_page_number?: boolean;
}

export interface PreeditSettingsSectionProps {
  preferences: PreeditSettingsPreferences;
  mobile: boolean;
  showShuangpinPreedit: boolean;
  showPageNumber?: boolean;
  inlinePreedit?: boolean;
  inlinePreeditBusy: boolean;
  onChange: (patch: Partial<PreeditSettingsPreferences>) => void;
  onInlinePreeditChange?: (enabled: boolean) => void;
}

/** Shared preedit presentation controls for physical and touch keyboard hosts: the rows of the 候选窗口 page's 预编辑 group. */
export function PreeditSettingsSection({
  preferences,
  mobile,
  showShuangpinPreedit,
  showPageNumber = false,
  inlinePreedit,
  inlinePreeditBusy,
  onChange,
  onInlinePreeditChange,
}: PreeditSettingsSectionProps) {
  return (
    <>
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
      <Row title={mobile ? "候选栏预编辑" : "候选窗预编辑"}>
        <PreeditStyleSelect
          mode="candidate"
          value={preferences.candidate_preedit_style ?? "pinyin"}
          onChange={(value) =>
            onChange({ candidate_preedit_style: value as CandidatePreeditStyle })
          }
        />
      </Row>
      {showPageNumber && (
        <SwitchRow
          title="显示页码"
          description="显示候选列表的当前页与总页数；关闭后仍可正常翻页。"
          checked={preferences.show_candidate_page_number !== false}
          onChange={(checked) => onChange({ show_candidate_page_number: checked })}
        />
      )}
    </>
  );
}
