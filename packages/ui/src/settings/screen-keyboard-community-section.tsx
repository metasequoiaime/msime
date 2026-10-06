import { SettingActionHeader } from "./setting-action-header";
import { ActionButton } from "./action-button";

export interface ScreenKeyboardCommunitySectionProps {
  onOpen: () => void;
}

/** Entry from the screen keyboard settings to the community skin catalog. */
export function ScreenKeyboardCommunitySection({ onOpen }: ScreenKeyboardCommunitySectionProps) {
  return (
    <div className="section">
      <SettingActionHeader title="社区皮肤" description="看看别人做的键盘皮肤，可以直接试用或保存">
        <ActionButton action={onOpen} label="去社区找皮肤" />
      </SettingActionHeader>
    </div>
  );
}
