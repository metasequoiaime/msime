import { SettingToggle } from "./setting-toggle";

export interface PunctuationPreferences {
  chinese_punctuation: boolean;
  character_width?: "halfwidth" | "fullwidth";
  smart_punctuation?: boolean;
  smart_punctuation_repeat?: boolean;
  smart_punctuation_space_convert?: boolean;
  smart_punctuation_direct_digit?: boolean;
  smart_punctuation_direct_letter?: boolean;
  paired_punctuation?: boolean;
  punctuation_lock?: "follow" | "chinese" | "english";
}

type PunctuationSwitch =
  | "smart_punctuation"
  | "smart_punctuation_repeat"
  | "smart_punctuation_space_convert"
  | "smart_punctuation_direct_digit"
  | "smart_punctuation_direct_letter"
  | "paired_punctuation";

const switches: readonly [PunctuationSwitch, string, string][] = [
  ["smart_punctuation", "智能标点", "中文标点模式下，字母或数字后的 , . : 自动使用英文标点"],
  [
    "smart_punctuation_repeat",
    "重复标点转中文",
    "智能标点输出英文标点后，2 秒内再次输入同一标点时替换为中文标点",
  ],
  [
    "smart_punctuation_space_convert",
    "中文标点后按空格转换",
    "刚输入中文标点后按空格，转换为对应英文标点",
  ],
  ["smart_punctuation_direct_digit", "数字后直出", "数字后输入逗号、句点或冒号时保留 ASCII 标点"],
  ["smart_punctuation_direct_letter", "字母后直出", "字母后输入逗号、句点或冒号时保留 ASCII 标点"],
  ["paired_punctuation", "成对标点自动补全", "输入左侧符号时自动补全右侧符号，并将光标置于中间"],
];

export interface PunctuationSectionProps {
  preferences: PunctuationPreferences;
  showCharacterWidth: boolean;
  onChange: (patch: Partial<PunctuationPreferences>) => void;
}

/** Shared input punctuation and character width controls. */
export function PunctuationSection({
  preferences,
  showCharacterWidth,
  onChange,
}: PunctuationSectionProps) {
  return (
    <>
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            中文标点<small>默认使用中文标点符号</small>
          </span>
          <input
            className="toggle"
            type="checkbox"
            checked={preferences.chinese_punctuation}
            onChange={(event) => onChange({ chinese_punctuation: event.target.checked })}
          />
        </label>
      </div>
      {showCharacterWidth && (
        <div className="section">
          <label className="section-header">
            <span className="section-title">
              全角输入
              <small>
                将英文字符和空格提交为全角形式，会话开始时生效；工具栏、键盘的更多工具或快捷键可临时切换
              </small>
            </span>
            <input
              aria-label="全角输入"
              className="toggle"
              type="checkbox"
              checked={(preferences.character_width ?? "halfwidth") === "fullwidth"}
              onChange={(event) =>
                onChange({ character_width: event.target.checked ? "fullwidth" : "halfwidth" })
              }
            />
          </label>
        </div>
      )}
      {switches.map(([key, label, description]) => (
        <SettingToggle
          key={key}
          label={label}
          description={description}
          checked={preferences[key] ?? key === "paired_punctuation"}
          onChange={(checked) => onChange({ [key]: checked })}
        />
      ))}
      <div className="section">
        <label className="section-header">
          <span className="section-title">
            固定标点<small>切换中英文时的标点形态，三者互斥</small>
          </span>
          <select
            aria-label="固定标点"
            value={preferences.punctuation_lock ?? "follow"}
            onChange={(event) =>
              onChange({
                punctuation_lock: event.target.value as NonNullable<
                  PunctuationPreferences["punctuation_lock"]
                >,
              })
            }
          >
            <option value="follow">跟随中英文状态</option>
            <option value="chinese">始终使用中文标点</option>
            <option value="english">始终使用英文标点</option>
          </select>
        </label>
      </div>
    </>
  );
}
