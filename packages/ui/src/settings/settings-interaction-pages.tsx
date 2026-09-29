import { HandwritingSettingsSection } from "./handwriting-settings-section";
import { ScreenKeyboardSettingsSection } from "./screen-keyboard-settings-section";

export interface SettingsInteractionPagesProps {
  screenKeyboard: Parameters<typeof ScreenKeyboardSettingsSection>[0];
  handwriting: Parameters<typeof HandwritingSettingsSection>[0];
  handwritingDisabled: boolean;
  handwritingHidden: boolean;
}

/** Groups the touch keyboard and handwriting interaction pages. */
export function SettingsInteractionPages({
  screenKeyboard,
  handwriting,
  handwritingDisabled,
  handwritingHidden,
}: SettingsInteractionPagesProps) {
  return (
    <>
      <ScreenKeyboardSettingsSection {...screenKeyboard} />
      <fieldset disabled={handwritingDisabled} hidden={handwritingHidden} aria-label="手写识别板">
        <HandwritingSettingsSection {...handwriting} />
      </fieldset>
    </>
  );
}
