import { SettingsRowStack } from "./settings-row-stack";
import { GroupList, Row, Select } from "../core/platform-controls";
import { EnglishSuggestionsSection } from "./english-suggestions-section";
import { SwitchRow } from "./switch-row";
import { ActionButton } from "./action-button";

import {
  mobileKeyboardFeedbackControlState,
  type MobileKeyboardFeedback,
} from "./mobile-keyboard-feedback";

export type {
  MobileKeyboardFeedback,
  MobileKeyboardFeedbackClient,
  SwipeSymbolsDirection,
  HandwritingStrokeColor,
} from "./mobile-keyboard-feedback";

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
      value={mobileKeyboardFeedbackControlState(value).englishSuggestionsChecked}
      onChange={(englishSuggestions) => onChange({ ...value, englishSuggestions })}
    />
  );
}

/** 共享的移动键盘声音、振动与 iOS 英文建议控件，组成「屏幕键盘」页的「按键反馈」组。 */
export function MobileKeyboardFeedbackSection({
  value,
  busy,
  ios,
  showEnglishSuggestions = ios,
  canPreview,
  onChange,
  onPreview,
}: MobileKeyboardFeedbackSectionProps) {
  const state = mobileKeyboardFeedbackControlState(value);
  return (
    <GroupList title="按键反馈">
      <SettingsRowStack role="group" aria-label="按键反馈">
        <SwitchRow
          title="按键音"
          description={
            ios
              ? "需要为键盘开启“允许完全访问”，并受系统键盘声音和静音设置控制"
              : "按键音跟随媒体音量"
          }
          disabled={busy}
          checked={value.soundEnabled}
          onChange={(checked) => onChange({ ...value, soundEnabled: checked })}
        />
        {state.hapticsVisible && (
          <SwitchRow
            title="按键振动"
            description="振动效果取决于设备与系统支持"
            disabled={busy}
            checked={value.hapticsEnabled}
            onChange={(checked) => onChange({ ...value, hapticsEnabled: checked })}
          />
        )}
        {value.keyPopup !== undefined && (
          <SwitchRow
            title="按键弹出预览"
            description="按下时在键上方显示放大的字符"
            disabled={busy}
            checked={value.keyPopup}
            onChange={(checked) => onChange({ ...value, keyPopup: checked })}
          />
        )}
        {state.hapticStrengthVisible && (
          <Row title="振动强度">
            {canPreview && <ActionButton action={onPreview} disabled={busy} label="试一下振动" />}
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
              <option value="system">跟随系统</option>
            </Select>
          </Row>
        )}
        {showEnglishSuggestions && (
          <MobileEnglishSuggestionsRow value={value} busy={busy} onChange={onChange} />
        )}
      </SettingsRowStack>
    </GroupList>
  );
}
