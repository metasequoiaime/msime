import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";

const navigationOptions: [keyof NavigationPreferences, string][] = [
  ["minus_equal", "- / ="],
  ["comma_period", ", / ."],
  ["brackets", "[ / ]"],
  ["tab", "Shift+Tab / Tab"],
  ["page_up_down", "PageUp / PageDown"],
  ["mouse_wheel", "鼠标滚轮（候选面板支持时翻页）"],
  ["arrows", "上 / 下（移动候选项）"],
];

const linuxWheelPagingNote =
  "鼠标滚轮：开启后在 IBus 候选窗口上滚动即翻页，关闭时滚轮不做任何事。Fcitx5 经典界面的滚轮翻页是 Fcitx5 自己的设置，开启或改回关闭后会同步写入，对 Fcitx5 中的所有输入法生效；从未开启过时沿用 Fcitx5 原有设置。";

export interface NavigationSectionProps {
  navigation: NavigationPreferences;
  wordCharacter: WordCharacterPreferences;
  linux: boolean;
  onChange: (next: {
    navigation: NavigationPreferences;
    wordCharacter: WordCharacterPreferences;
  }) => void;
}

/** Shared candidate paging controls and their mutual exclusion with 以词定字. */
export function NavigationSection({
  navigation,
  wordCharacter,
  linux,
  onChange,
}: NavigationSectionProps) {
  return (
    <div className="section" role="group" aria-labelledby="paging-title">
      <div className="section-title" id="paging-title">
        翻页方式
      </div>
      <div className="input-option-content">
        {navigationOptions.map(([key, label], index) => (
          <div className="input-option-item" key={key}>
            {index > 0 && <div className="input-option-divider" />}
            <label className="check-option">
              <input
                type="checkbox"
                checked={navigation[key]}
                onChange={(event) => {
                  const enabled = event.target.checked;
                  onChange({
                    navigation: { ...navigation, [key]: enabled },
                    wordCharacter:
                      enabled && wordCharacter.enabled && wordCharacter.keys === key
                        ? { ...wordCharacter, enabled: false }
                        : wordCharacter,
                  });
                }}
              />
              <span>{label}</span>
            </label>
          </div>
        ))}
      </div>
      {linux && <div className="input-setting-description">{linuxWheelPagingNote}</div>}
    </div>
  );
}
