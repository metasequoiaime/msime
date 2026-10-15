import type {
  PunctuationPreferences as SharedPunctuationPreferences,
  QuoteStyle,
  BracketStyle,
} from "./punctuation-preferences";
import { punctuationControlState } from "./punctuation-preferences";
import {
  smartPunctuation,
  smartPunctuationRefinements,
  capsLockPunctuation,
  punctuationSwitches,
  punctuationLockOptions,
  quoteStyleOptions,
  bracketStyleOptions,
  type PunctuationSwitch,
} from "./punctuation-options";
import { MoreOptions } from "../core/platform-controls";
import { SelectRow } from "./select-row";
import { SwitchRow } from "./switch-row";

export type { QuoteStyle, BracketStyle } from "./punctuation-preferences";

export interface PunctuationPreferences extends SharedPunctuationPreferences {}

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
      checked={punctuationControlState(preferences).character_width === "fullwidth"}
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
      <QuoteStyleRow preferences={preferences} onChange={onChange} />
      <BracketStyleRow preferences={preferences} onChange={onChange} />
      {showCharacterWidth && <CharacterWidthRow preferences={preferences} onChange={onChange} />}
      {punctuationSwitches.map(([key, label, description]) => (
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
      checked={punctuationControlState(preferences)[preference]}
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
      value={punctuationControlState(preferences).punctuation_lock}
      onChange={(event) =>
        onChange({
          punctuation_lock: event.target.value as NonNullable<
            PunctuationPreferences["punctuation_lock"]
          >,
        })
      }
    >
      {punctuationLockOptions.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
}

/** 引号样式：只在中文标点下起作用，所以紧跟在「中文标点」之后。直角引号按键对应，说明里写出键位。 */
export function QuoteStyleRow({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
    <SelectRow
      title="引号样式"
      description="中文标点下 ' 键和 Shift+' 键输出的引号。直角引号：' 出「」，Shift+' 出『』"
      value={punctuationControlState(preferences).quote_style}
      onChange={(event) => onChange({ quote_style: event.target.value as QuoteStyle })}
    >
      {quoteStyleOptions.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
}

/** 方头括号样式：`[` `]` 出实心【】还是空心〖〗。 */
export function BracketStyleRow({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
    <SelectRow
      title="方头括号样式"
      description="中文标点下 [ ] 键输出的括号"
      value={punctuationControlState(preferences).bracket_style}
      onChange={(event) => onChange({ bracket_style: event.target.value as BracketStyle })}
    >
      {bracketStyleOptions.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
}

/** HarmonyOS 手机「表达」页的「标点」分组，遵循设计稿和 Android 的 `ExpressionPage`：「使用英文标点」（显示为 `chinese_punctuation` 的反值）、「自动补全成对标点」和「智能标点」，引号与方头括号样式、智能标点的细分选项和「固定标点」收在「更多选项」下，不丢失任何生效中的设置。 */
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
        <QuoteStyleRow preferences={preferences} onChange={onChange} />
        <BracketStyleRow preferences={preferences} onChange={onChange} />
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
