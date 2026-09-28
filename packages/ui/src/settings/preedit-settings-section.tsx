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

/** Shared preedit presentation controls for physical and touch keyboard hosts. */
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
        <div className="section">
          <label className="section-header">
            <span className="section-title">
              双拼预编辑
              <small>仅在双拼方案下生效；选择保留原始双拼按键，或显示展开后的拼音分词。</small>
            </span>
            <select
              aria-label="双拼预编辑"
              value={preferences.shuangpin_preedit_uses_raw === false ? "pinyin" : "raw"}
              onChange={(event) =>
                onChange({ shuangpin_preedit_uses_raw: event.target.value === "raw" })
              }
            >
              <option value="raw">原始按键</option>
              <option value="pinyin">拼音分词</option>
            </select>
          </label>
        </div>
      )}
      {inlinePreedit !== undefined ? (
        <div className="section">
          <label className="section-header">
            <span className="section-title">
              行内预编辑
              <small>
                把正在拼写的编码也写进输入框，像系统键盘那样带下划线显示。默认关闭；个别 App
                显示不完整时可以关掉。
              </small>
            </span>
            <input
              aria-label="行内预编辑"
              className="toggle"
              type="checkbox"
              disabled={inlinePreeditBusy}
              checked={inlinePreedit}
              onChange={(event) => onInlinePreeditChange?.(event.target.checked)}
            />
          </label>
        </div>
      ) : (
        <div className="section">
          <label className="section-header">
            <span className="section-title">行内预编辑</span>
            <select
              aria-label="行内预编辑"
              value={preferences.tsf_preedit_style ?? "raw"}
              onChange={(event) =>
                onChange({ tsf_preedit_style: event.target.value as TsfPreeditStyle })
              }
            >
              <option value="raw">原始按键</option>
              <option value="pinyin">拼音分词</option>
              <option value="empty">不显示</option>
            </select>
          </label>
        </div>
      )}
      <div className="section">
        <label className="section-header">
          <span className="section-title">{mobile ? "候选栏预编辑" : "候选窗预编辑"}</span>
          <select
            aria-label={mobile ? "候选栏预编辑" : "候选窗预编辑"}
            value={preferences.candidate_preedit_style ?? "pinyin"}
            onChange={(event) =>
              onChange({ candidate_preedit_style: event.target.value as CandidatePreeditStyle })
            }
          >
            <option value="pinyin">拼音分词</option>
            <option value="empty">不显示</option>
          </select>
        </label>
      </div>
    </>
  );
}
