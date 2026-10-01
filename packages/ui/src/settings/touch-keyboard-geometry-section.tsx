import { Checks, GroupList, Row, Slider, Switch } from "../core/platform-controls";
import * as settings from "./settings-style";

export type TouchToolbarPreferences = {
  layout: boolean;
  emoji: boolean;
  skin: boolean;
  clipboard: boolean;
  ai: boolean;
  character_set: boolean;
  fullwidth: boolean;
  punctuation: boolean;
};

const defaultTouchToolbar: TouchToolbarPreferences = {
  layout: true,
  emoji: true,
  skin: true,
  clipboard: false,
  ai: false,
  character_set: false,
  fullwidth: false,
  punctuation: false,
};

/// In the order the buttons sit on the touch keyboard's toolbar, after the voice entry.
const touchToolbarOptions: readonly [keyof TouchToolbarPreferences, string][] = [
  ["layout", "键盘设置"],
  ["emoji", "表情"],
  ["skin", "切换皮肤"],
  ["clipboard", "剪贴板历史"],
  ["ai", "AI 润色"],
  ["character_set", "简繁切换"],
  ["fullwidth", "全角 / 半角"],
  ["punctuation", "中英文标点"],
];

export interface TouchKeyboardGeometrySectionProps {
  heightAdjustment: number;
  keySpacingTenths: number;
  rowSpacingTenths: number;
  touchVoiceShortcut: boolean;
  toolbarComponents: boolean;
  toolbar?: Partial<TouchToolbarPreferences>;
  tabletFullKeys?: boolean;
  tabletFullKeysBusy: boolean;
  onHeightAdjustmentChange: (value: number) => void;
  onKeySpacingChange: (value: number) => void;
  onRowSpacingChange: (value: number) => void;
  onTouchVoiceShortcutChange: (enabled: boolean) => void;
  onToolbarChange: (value: TouchToolbarPreferences) => void;
  onTabletFullKeysChange: (enabled: boolean) => void;
  onReset: () => void;
}

/** Shared touch keyboard controls for the 屏幕键盘 page, as the groups that follow its preview: 尺寸 (height, spacing and the reset), 工具栏, and 布局 where the host has the iPad's digit row and Tab key. */
export function TouchKeyboardGeometrySection({
  heightAdjustment,
  keySpacingTenths,
  rowSpacingTenths,
  touchVoiceShortcut,
  toolbarComponents,
  toolbar,
  tabletFullKeys,
  tabletFullKeysBusy,
  onHeightAdjustmentChange,
  onKeySpacingChange,
  onRowSpacingChange,
  onTouchVoiceShortcutChange,
  onToolbarChange,
  onTabletFullKeysChange,
  onReset,
}: TouchKeyboardGeometrySectionProps) {
  const toolbarValues = { ...defaultTouchToolbar, ...toolbar };

  return (
    <>
      <GroupList title="尺寸">
        <p className={settings.groupNote}>
          只改变触屏键位的外观，不改变输入方案；也可以直接在上方预览上左右拖动调节键距、上下拖动调节行距。
        </p>
        <Row
          title="键盘高度"
          description={`${heightAdjustment > 0 ? "+" : ""}${heightAdjustment} dp`}
        >
          <span className={settings.sliderControl}>
            <Slider
              min={-12}
              max={48}
              value={heightAdjustment}
              onChange={onHeightAdjustmentChange}
            />
          </span>
        </Row>
        <Row title="按键间距" description={`${(keySpacingTenths / 10).toFixed(1)} dp`}>
          <span className={settings.sliderControl}>
            <Slider min={30} max={60} value={keySpacingTenths} onChange={onKeySpacingChange} />
          </span>
        </Row>
        <Row title="行间距" description={`${(rowSpacingTenths / 10).toFixed(1)} dp`}>
          <span className={settings.sliderControl}>
            <Slider min={40} max={100} value={rowSpacingTenths} onChange={onRowSpacingChange} />
          </span>
        </Row>
        {/* The reset also covers the 工具栏 group below, as it always has; it sits here, at the end of the first of the groups it resets. */}
        <Row title="恢复默认" description="高度、间距、顶部语音入口和工具栏按钮回到默认">
          <button
            type="button"
            className="danger-text"
            aria-label="恢复屏幕键盘默认设置"
            onClick={onReset}
          >
            恢复默认
          </button>
        </Row>
      </GroupList>
      <GroupList title="工具栏">
        <Row title="顶部语音入口" description="在触屏键盘工具栏直接打开最近一次语音结果">
          <Switch checked={touchVoiceShortcut} onChange={onTouchVoiceShortcutChange} />
        </Row>
        {toolbarComponents && (
          <div className={settings.groupBlock}>
            <Checks
              legend="工具栏按钮"
              description="勾选要显示在键盘顶部工具栏的功能；未勾选的仍在「更多」里"
              items={touchToolbarOptions.map(([key, label]) => ({
                value: key,
                // The hidden prefix keeps each box named "工具栏：…" for assistive technology, as the old per-box label did.
                label: (
                  <>
                    <span className="sr-only">工具栏：</span>
                    {label}
                  </>
                ),
                checked: toolbarValues[key],
              }))}
              onChange={(key, checked) => onToolbarChange({ ...toolbarValues, [key]: checked })}
            />
          </div>
        )}
      </GroupList>
      {tabletFullKeys !== undefined && (
        <GroupList title="布局">
          <Row
            title="数字行与 Tab 键"
            description="iPad 全宽键盘在字母上方显示数字行，并在 Q 左侧显示 Tab 键；浮动键盘和窄窗口没有空间，不显示。"
          >
            <Switch
              disabled={tabletFullKeysBusy}
              checked={tabletFullKeys}
              onChange={onTabletFullKeysChange}
            />
          </Row>
        </GroupList>
      )}
    </>
  );
}
