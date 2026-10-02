import { SettingActionHeader } from "./setting-action-header";

export interface ScreenKeyboardCommunitySectionProps {
  onOpen: () => void;
}

/** Entry from the screen keyboard settings to the community skin catalog. */
export function ScreenKeyboardCommunitySection({ onOpen }: ScreenKeyboardCommunitySectionProps) {
  return (
    <div className="section">
      <SettingActionHeader title="社区皮肤" description="看看别人做的键盘皮肤，可以直接试用或保存">
        <button type="button" className="secondary" onClick={onOpen}>
          去社区找皮肤
        </button>
      </SettingActionHeader>
    </div>
  );
}
