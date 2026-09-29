export interface ScreenKeyboardCommunitySectionProps {
  onOpen: () => void;
}

/** Entry from the screen keyboard settings to the community skin catalog. */
export function ScreenKeyboardCommunitySection({ onOpen }: ScreenKeyboardCommunitySectionProps) {
  return (
    <div className="section">
      <div className="section-header">
        <span className="section-title">
          社区皮肤<small>看看别人做的键盘皮肤，可以直接试用或保存</small>
        </span>
        <button type="button" className="secondary" onClick={onOpen}>
          去社区发现皮肤
        </button>
      </div>
    </div>
  );
}
