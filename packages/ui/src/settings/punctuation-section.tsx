import { MoreOptions } from "../core/platform-controls";
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
  caps_lock_ascii_punctuation?: boolean;
}

type PunctuationSwitch =
  | "smart_punctuation"
  | "smart_punctuation_repeat"
  | "smart_punctuation_space_convert"
  | "smart_punctuation_direct_digit"
  | "smart_punctuation_direct_letter"
  | "paired_punctuation"
  | "caps_lock_ascii_punctuation";

type PunctuationSwitchCopy = readonly [PunctuationSwitch, string, string];

const smartPunctuation: PunctuationSwitchCopy = [
  "smart_punctuation",
  "智能标点",
  "中文标点模式下，字母或数字后的 , . : 自动使用英文标点",
];

/** 「智能标点」的细分选项，紧接在它之后列出。 */
const smartPunctuationRefinements: readonly PunctuationSwitchCopy[] = [
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
];

const pairedPunctuation: PunctuationSwitchCopy = [
  "paired_punctuation",
  "成对标点自动补全",
  "输入左侧符号时自动补全右侧符号，并将光标置于中间",
];

/** 只在报告大写锁定状态的宿主上显示（`HostCapabilities.caps_lock_punctuation`）。 */
const capsLockPunctuation: PunctuationSwitchCopy = [
  "caps_lock_ascii_punctuation",
  "大写锁定时使用英文标点",
  "大写锁定打开时，中文输入下的标点使用英文标点；正在组字或固定中文标点时不变",
];

const switches: readonly PunctuationSwitchCopy[] = [
  smartPunctuation,
  ...smartPunctuationRefinements,
  pairedPunctuation,
];

export interface PunctuationSectionProps {
  preferences: PunctuationPreferences;
  /** Draws the 全角输入 row after 中文标点. The settings form keeps that row with the other output settings on the 输入 page, so its 标点 group leaves this off and renders `CharacterWidthRow` there instead. */
  showCharacterWidth: boolean;
  /** 显示「大写锁定时使用英文标点」一行；只有向会话报告大写锁定的宿主（`HostCapabilities.caps_lock_punctuation`）才用得上。 */
  showCapsLockPunctuation?: boolean;
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
  showCapsLockPunctuation = false,
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
        <PunctuationSwitchRow
          key={key}
          preference={key}
          title={label}
          description={description}
          preferences={preferences}
          onChange={onChange}
        />
      ))}
      {showCapsLockPunctuation && (
        <PunctuationSwitchRow
          preference={capsLockPunctuation[0]}
          title={capsLockPunctuation[1]}
          description={capsLockPunctuation[2]}
          preferences={preferences}
          onChange={onChange}
        />
      )}
      <PunctuationLockRow preferences={preferences} onChange={onChange} />
    </>
  );
}

/** 某个标点开关的存储值；偏好不存在时只有「成对标点」是开启的。 */
function punctuationSwitchChecked(preferences: PunctuationPreferences, key: PunctuationSwitch) {
  return preferences[key] ?? key === "paired_punctuation";
}

function PunctuationSwitchRow({
  preference,
  title,
  description,
  preferences,
  onChange,
}: {
  preference: PunctuationSwitch;
  title: string;
  description?: string;
} & Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
    <SwitchRow
      title={title}
      description={description}
      checked={punctuationSwitchChecked(preferences, preference)}
      onChange={(checked) => onChange({ [preference]: checked })}
    />
  );
}

function PunctuationLockRow({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
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
  );
}

/** HarmonyOS 手机「表达」页的「标点」分组，遵循设计稿和 Android 的 `ExpressionPage`：「使用英文标点」（显示为 `chinese_punctuation` 的反值）、「自动补全成对标点」和「智能标点」，智能标点的细分选项和「固定标点」收在「更多选项」下，不丢失任何生效中的设置。 */
export function PhonePunctuationSection({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  const [smartKey, smartTitle, smartDescription] = smartPunctuation;
  return (
    <>
      <SwitchRow
        title="使用英文标点"
        checked={!preferences.chinese_punctuation}
        onChange={(checked) => onChange({ chinese_punctuation: !checked })}
      />
      <PunctuationSwitchRow
        preference="paired_punctuation"
        title="自动补全成对标点"
        preferences={preferences}
        onChange={onChange}
      />
      <PunctuationSwitchRow
        preference={smartKey}
        title={smartTitle}
        description={smartDescription}
        preferences={preferences}
        onChange={onChange}
      />
      <MoreOptions>
        {smartPunctuationRefinements.map(([key, label, description]) => (
          <PunctuationSwitchRow
            key={key}
            preference={key}
            title={label}
            description={description}
            preferences={preferences}
            onChange={onChange}
          />
        ))}
        <PunctuationLockRow preferences={preferences} onChange={onChange} />
      </MoreOptions>
    </>
  );
}
