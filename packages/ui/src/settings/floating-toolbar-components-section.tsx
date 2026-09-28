import * as settings from "./settings-style";

export type FloatingToolbarComponentKey =
  | "english_mode"
  | "fullwidth"
  | "punctuation"
  | "character_set"
  | "emoji"
  | "handwriting"
  | "screen_keyboard"
  | "voice"
  | "settings";
export type FloatingToolbarCapability = "floating_toolbar_handwriting" | "floating_toolbar_voice";

export interface FloatingToolbarComponentsSectionProps {
  values: Record<FloatingToolbarComponentKey, boolean>;
  capabilities?: Partial<Record<FloatingToolbarCapability, boolean>>;
  onChange: (key: FloatingToolbarComponentKey, enabled: boolean) => void;
}

const options: [FloatingToolbarComponentKey, string, FloatingToolbarCapability | null][] = [
  ["english_mode", "英文输入模式", null],
  ["fullwidth", "全角 / 半角", null],
  ["punctuation", "中英文标点", null],
  ["character_set", "简繁切换", null],
  ["emoji", "表情与符号", null],
  ["handwriting", "手写识别板", "floating_toolbar_handwriting"],
  ["screen_keyboard", "屏幕键盘", null],
  ["voice", "语音输入", "floating_toolbar_voice"],
  ["settings", "设置", null],
];

/** Capability-aware component switches for the floating toolbar. */
export function FloatingToolbarComponentsSection({
  values,
  capabilities,
  onChange,
}: FloatingToolbarComponentsSectionProps) {
  const visibleOptions = options.filter(
    ([, , capability]) => !capability || !capabilities || capabilities[capability],
  );
  return (
    <div className={`section ${settings.toolbarComponents}`}>
      <div className="section-title">
        工具栏组件<small>勾选要显示在悬浮工具栏中的功能</small>
      </div>
      <div className={settings.toolbarComponentList}>
        <label className={`check-option ${settings.toolbarRequiredOption}`}>
          <input type="checkbox" checked disabled />
          <span>中英文切换</span>
          <span className={settings.toolbarRequiredLabel}>始终显示</span>
        </label>
        {visibleOptions.map(([key, label]) => (
          <div key={key}>
            <div className="input-option-divider" />
            <label className="check-option">
              <input
                type="checkbox"
                checked={values[key]}
                onChange={(event) => onChange(key, event.target.checked)}
              />
              <span>{label}</span>
            </label>
          </div>
        ))}
      </div>
    </div>
  );
}
