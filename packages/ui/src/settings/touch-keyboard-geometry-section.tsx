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

/** Shared touch keyboard size, toolbar, and reset controls. */
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
    <div className="section" role="group" aria-labelledby="touch-keyboard-geometry-title">
      <div className="section-title" id="touch-keyboard-geometry-title">
        触屏键盘尺寸
        <small>
          与 Apple 键盘一致，只改变触屏键位外观，不改变输入方案或 Engine
          组合状态；也可以直接在下方预览上左右拖动调节键距、上下拖动调节行距。
        </small>
      </div>
      <label className="section-header">
        <span className="section-title">
          键盘高度{" "}
          <small>
            {heightAdjustment > 0 ? "+" : ""}
            {heightAdjustment} dp
          </small>
        </span>
        <input
          aria-label="键盘高度"
          type="range"
          min="-12"
          max="48"
          step="1"
          value={heightAdjustment}
          onChange={(event) => onHeightAdjustmentChange(Number(event.target.value))}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">
          按键间距 <small>{(keySpacingTenths / 10).toFixed(1)} dp</small>
        </span>
        <input
          aria-label="按键间距"
          type="range"
          min="30"
          max="60"
          step="1"
          value={keySpacingTenths}
          onChange={(event) => onKeySpacingChange(Number(event.target.value))}
        />
      </label>
      <div className="input-option-divider" />
      <label className="section-header">
        <span className="section-title">
          行间距 <small>{(rowSpacingTenths / 10).toFixed(1)} dp</small>
        </span>
        <input
          aria-label="行间距"
          type="range"
          min="40"
          max="100"
          step="1"
          value={rowSpacingTenths}
          onChange={(event) => onRowSpacingChange(Number(event.target.value))}
        />
      </label>
      <div className="input-option-divider" />
      <SettingToggle
        label="顶部语音入口"
        description="在触屏键盘工具栏直接打开最近一次语音结果"
        ariaLabel="顶部语音入口"
        checked={touchVoiceShortcut}
        compact
        onChange={onTouchVoiceShortcutChange}
      />
      {toolbarComponents && (
        <>
          <div className="input-option-divider" />
          <div className="section-title">
            工具栏按钮
            <small>勾选要显示在键盘顶部工具栏的功能；未勾选的仍在「更多」里</small>
          </div>
          {touchToolbarOptions.map(([key, label]) => (
            <label key={key} className="check-option">
              <input
                type="checkbox"
                aria-label={`工具栏：${label}`}
                checked={toolbarValues[key]}
                onChange={(event) =>
                  onToolbarChange({ ...toolbarValues, [key]: event.target.checked })
                }
              />
              <span>{label}</span>
            </label>
          ))}
        </>
      )}
      {tabletFullKeys !== undefined && (
        <>
          <div className="input-option-divider" />
          <SettingToggle
            label="数字行与 Tab 键"
            description="iPad 全宽键盘在字母上方显示数字行，并在 Q 左侧显示 Tab 键；浮动键盘和窄窗口没有空间，不显示。"
            ariaLabel="数字行与 Tab 键"
            disabled={tabletFullKeysBusy}
            checked={tabletFullKeys}
            compact
            onChange={onTabletFullKeysChange}
          />
        </>
      )}
      <button
        type="button"
        className="danger-text"
        aria-label="恢复屏幕键盘默认设置"
        onClick={onReset}
      >
        恢复默认
      </button>
    </div>
  );
}
import { SettingToggle } from "./setting-toggle";
