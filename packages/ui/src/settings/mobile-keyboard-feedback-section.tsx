export type MobileKeyboardFeedback = {
  soundEnabled: boolean;
  hapticsEnabled: boolean;
  hapticStrength: "light" | "medium" | "strong";
  /** iOS keeps this Apple keyboard preference in the native App Group store. */
  englishSuggestions?: boolean;
  /** iOS draws the candidate strip in the keyboard skin unless this App Group switch hands it to the shared candidate skin and colours. */
  candidatePaletteFollowsDesktop?: boolean;
  /** iOS writes the composition into the text field as marked text only when this App Group switch is on; it has no raw/pinyin/empty choice because the strip already carries that one. */
  inlinePreedit?: boolean;
  /** False where the device cannot vibrate for key presses (iPad has no Taptic Engine): the vibration controls are hidden and the stored choice is left for the user's other devices. */
  hapticsAvailable?: boolean;
  /** iPad only: the digit row and Tab key of the full-width keyboard, kept in the App Group. Absent on a phone, where the keyboard has no room for either. */
  tabletFullKeys?: boolean;
};

export type MobileKeyboardFeedbackClient = {
  load(): Promise<MobileKeyboardFeedback>;
  save(settings: MobileKeyboardFeedback): Promise<MobileKeyboardFeedback>;
  preview?(strength: MobileKeyboardFeedback["hapticStrength"]): Promise<void>;
};

export interface MobileKeyboardFeedbackSectionProps {
  value: MobileKeyboardFeedback;
  busy: boolean;
  ios: boolean;
  canPreview: boolean;
  onChange: (value: MobileKeyboardFeedback) => void;
  onPreview: () => void;
}

/** Shared mobile keyboard sound, haptic, and iOS English suggestion controls. */
export function MobileKeyboardFeedbackSection({
  value,
  busy,
  ios,
  canPreview,
  onChange,
  onPreview,
}: MobileKeyboardFeedbackSectionProps) {
  return (
    <div className="section" role="group" aria-label="按键反馈">
      <div className="section-title">按键反馈</div>
      <SettingToggle
        label="按键音"
        description="按键音受系统静音设置控制"
        ariaLabel="按键音"
        disabled={busy}
        checked={value.soundEnabled}
        compact
        onChange={(enabled) => onChange({ ...value, soundEnabled: enabled })}
      />
      {value.hapticsAvailable !== false && (
        <>
          <div className="input-option-divider" />
          <SettingToggle
            label="按键振动"
            description="振动效果取决于设备与系统支持"
            ariaLabel="按键振动"
            disabled={busy}
            checked={value.hapticsEnabled}
            compact
            onChange={(enabled) => onChange({ ...value, hapticsEnabled: enabled })}
          />
          {value.hapticsEnabled && (
            <>
              <div className="input-option-divider" />
              <label className="section-header">
                <span className="section-title">振动强度</span>
                <select
                  aria-label="振动强度"
                  disabled={busy}
                  value={value.hapticStrength}
                  onChange={(event) =>
                    onChange({
                      ...value,
                      hapticStrength: event.target
                        .value as MobileKeyboardFeedback["hapticStrength"],
                    })
                  }
                >
                  <option value="light">轻</option>
                  <option value="medium">中</option>
                  <option value="strong">强</option>
                </select>
              </label>
              {canPreview && (
                <button type="button" className="secondary" disabled={busy} onClick={onPreview}>
                  试一下振动
                </button>
              )}
            </>
          )}
        </>
      )}
      {ios && (
        <>
          <div className="input-option-divider" />
          <SettingToggle
            label="英文建议"
            description="英文 26 键直接输入时，在候选栏显示当前单词的补全建议；关闭后仍可正常输入英文。"
            ariaLabel="英文建议"
            disabled={busy}
            checked={value.englishSuggestions !== false}
            compact
            onChange={(enabled) => onChange({ ...value, englishSuggestions: enabled })}
          />
        </>
      )}
    </div>
  );
}
import { SettingToggle } from "./setting-toggle";
