import * as settings from "./settings-style";
import { GroupList, Row, Select } from "../core/platform-controls";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { SwitchRow } from "./switch-row";

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
  /** 是否在这一组里画 iOS 的「英文建议」，默认跟随 `ios`。设置窗口把它放在输入页「候选与联想」组，所以屏幕键盘页传 false。 */
  showEnglishSuggestions?: boolean;
  canPreview: boolean;
  onChange: (value: MobileKeyboardFeedback) => void;
  onPreview: () => void;
}

/** iOS 键盘的「英文建议」开关，存在原生 App Group 里，而不是共享偏好的 `english_suggestions`。 */
export function MobileEnglishSuggestionsRow({
  value,
  busy,
  onChange,
}: Pick<MobileKeyboardFeedbackSectionProps, "value" | "busy" | "onChange">) {
  return (
    <EnglishSuggestionsSection
      disabled={busy}
      value={value.englishSuggestions !== false}
      onChange={(englishSuggestions) => onChange({ ...value, englishSuggestions })}
    />
  );
}

/** Shared mobile keyboard sound, haptic, and iOS English suggestion controls: the 屏幕键盘 page's 按键反馈 group. */
export function MobileKeyboardFeedbackSection({
  value,
  busy,
  ios,
  showEnglishSuggestions = ios,
  canPreview,
  onChange,
  onPreview,
}: MobileKeyboardFeedbackSectionProps) {
  return (
    <GroupList title="按键反馈">
      <div className={settings.rowStack} role="group" aria-label="按键反馈">
        <SwitchRow
          title="按键音"
          description="按键音受系统静音设置控制"
          disabled={busy}
          checked={value.soundEnabled}
          onChange={(checked) => onChange({ ...value, soundEnabled: checked })}
        />
        {value.hapticsAvailable !== false && (
          <SwitchRow
            title="按键振动"
            description="振动效果取决于设备与系统支持"
            disabled={busy}
            checked={value.hapticsEnabled}
            onChange={(checked) => onChange({ ...value, hapticsEnabled: checked })}
          />
        )}
        {value.hapticsAvailable !== false && value.hapticsEnabled && (
          <Row title="振动强度">
            {canPreview && (
              <button type="button" className="secondary" disabled={busy} onClick={onPreview}>
                试一下振动
              </button>
            )}
            <Select
              disabled={busy}
              value={value.hapticStrength}
              onChange={(event) =>
                onChange({
                  ...value,
                  hapticStrength: event.target.value as MobileKeyboardFeedback["hapticStrength"],
                })
              }
            >
              <option value="light">轻</option>
              <option value="medium">中</option>
              <option value="strong">强</option>
            </Select>
          </Row>
        )}
        {showEnglishSuggestions && (
          <MobileEnglishSuggestionsRow value={value} busy={busy} onChange={onChange} />
        )}
      </div>
    </GroupList>
  );
}
