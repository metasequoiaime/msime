import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";

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
  /** Draws the 全角输入 row after 中文标点. The settings form keeps that row with the other output settings on the 输入 page, so its 标点 group leaves this off and renders `CharacterWidthRow` there instead. */
  showCharacterWidth: boolean;
  onChange: (patch: Partial<PunctuationPreferences>) => void;
}

/** The 全角输入 switch: one row of the group it is placed in. */
export function CharacterWidthRow({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
    <SwitchRow
      title="全角输入"
      description="将英文字符和空格提交为全角形式，会话开始时生效；工具栏、键盘的更多工具或快捷键可临时切换"
      checked={(preferences.character_width ?? "halfwidth") === "fullwidth"}
      onChange={(checked) => onChange({ character_width: checked ? "fullwidth" : "halfwidth" })}
    />
  );
}

/** Shared punctuation controls: the rows of the 标点 group. */
export function PunctuationSection({
  preferences,
  showCharacterWidth,
  onChange,
}: PunctuationSectionProps) {
  return (
    <>
      <SwitchRow
        title="中文标点"
        description="默认使用中文标点符号"
        checked={preferences.chinese_punctuation}
        onChange={(chinese_punctuation) => onChange({ chinese_punctuation })}
      />
      {showCharacterWidth && <CharacterWidthRow preferences={preferences} onChange={onChange} />}
      {switches.map(([key, label, description]) => (
        <SwitchRow
          key={key}
          title={label}
          description={description}
          checked={preferences[key] ?? key === "paired_punctuation"}
          onChange={(checked) => onChange({ [key]: checked })}
        />
      ))}
      <SelectRow
        title="固定标点"
        description="切换中英文时的标点形态，三者互斥"
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
      </SelectRow>
    </>
  );
}
