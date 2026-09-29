import { Row, Select, Switch } from "../core/platform-controls";

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

/** Shared preedit presentation controls for physical and touch keyboard hosts: the rows of the 候选窗口 page's 预编辑 group. */
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
      {showShuangpinPreedit && (
        <Row
          title="双拼预编辑"
          description="仅在双拼方案下生效；选择保留原始双拼按键，或显示展开后的拼音分词。"
        >
          <Select
            value={preferences.shuangpin_preedit_uses_raw === false ? "pinyin" : "raw"}
            onChange={(event) =>
              onChange({ shuangpin_preedit_uses_raw: event.target.value === "raw" })
            }
          >
            <option value="raw">原始按键</option>
            <option value="pinyin">拼音分词</option>
          </Select>
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
          <Select
            value={preferences.tsf_preedit_style ?? "raw"}
            onChange={(event) =>
              onChange({ tsf_preedit_style: event.target.value as TsfPreeditStyle })
            }
          >
            <option value="raw">原始按键</option>
            <option value="pinyin">拼音分词</option>
            <option value="empty">不显示</option>
          </Select>
        </Row>
      )}
      <Row title={mobile ? "候选栏预编辑" : "候选窗预编辑"}>
        <Select
          value={preferences.candidate_preedit_style ?? "pinyin"}
          onChange={(event) =>
            onChange({ candidate_preedit_style: event.target.value as CandidatePreeditStyle })
          }
        >
          <option value="pinyin">拼音分词</option>
          <option value="empty">不显示</option>
        </Select>
      </Row>
    </>
  );
}
