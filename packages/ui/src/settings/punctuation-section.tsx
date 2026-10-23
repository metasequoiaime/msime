import {
  punctuationControlState,
  enumerationCommaPatch,
  punctuationMode,
  punctuationModePatch,
  smartPunctuationPatch,
  type PunctuationMode,
  type PunctuationPreferences as SharedPunctuationPreferences,
  type QuoteStyle,
  type BracketStyle,
} from "./punctuation-preferences";
import {
  smartPunctuation,
  smartPunctuationRefinements,
  capsLockPunctuation,
  enumerationCommaSwitches,
  pairedPunctuation,
  punctuationLockOptions,
  punctuationModeOptions,
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
  /** 在「标点」和「引号样式」之后画出「全角输入」一行。设置表单把这一行和其他输出设置一起放在输入页，所以这里的「标点」分组不开它，改在输入页渲染 `CharacterWidthRow`。 */
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

/** 桌面和 Android、iOS 共用的「标点」分组。主流程只留三行：「标点」选择（合并了「中文标点」开关和「固定标点」）、引号样式、成对标点；方头括号样式、智能标点总开关和它的四个细分开关、大写锁定标点收进「更多选项」。智能标点默认开（数字后的 `.` 直接出英文），用户不必先找到它才得到这个行为，所以总开关也不占主流程，和细分开关挨在一起。细分开关只在智能标点打开时才起作用（数字后、字母后直出见 `punctuation::route`，重复和空格转换见 macOS `InputController` 的智能标点分支），所以总开关关着时它们变灰，值保留不动。 */
export function PunctuationSection({
  preferences,
  showCharacterWidth,
  showCapsLockPunctuation = false,
  onChange,
}: PunctuationSectionProps) {
  const state = punctuationControlState(preferences);
  const smartEnabled = state.smart_punctuation;
  return (
    <>
      <PunctuationModeRow preferences={preferences} onChange={onChange} />
      <QuoteStyleRow preferences={preferences} onChange={onChange} />
      {showCharacterWidth && <CharacterWidthRow preferences={preferences} onChange={onChange} />}
      <PunctuationSwitchRow
        preference={pairedPunctuation[0]}
        title={pairedPunctuation[1]}
        description={pairedPunctuation[2]}
        preferences={preferences}
        onChange={onChange}
      />
      <MoreOptions>
        <BracketStyleRow preferences={preferences} onChange={onChange} />
        {enumerationCommaSwitches.map(([key, label, description]) => (
          <PunctuationSwitchRow
            key={key}
            preference={key}
            title={label}
            description={description}
            disabled={
              !state[
                key === "enumeration_comma_backslash"
                  ? "enumeration_comma_slash"
                  : "enumeration_comma_backslash"
              ]
            }
            preferences={preferences}
            onChange={onChange}
          />
        ))}
        <SmartPunctuationRow preferences={preferences} onChange={onChange} />
        {smartPunctuationRefinements.map(([key, label, description]) => (
          <PunctuationSwitchRow
            key={key}
            preference={key}
            title={label}
            description={description}
            disabled={!smartEnabled}
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
      </MoreOptions>
    </>
  );
}

/** 「标点」选择：显示值每次从 `chinese_punctuation` 和 `punctuation_lock` 推出，打开页面不改写文档；只有用户选了一项才把两个键一起写入。 */
function PunctuationModeRow({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
    <SelectRow
      title="标点"
      description="中文模式下使用的标点。选中文标点或英文标点时，可用快捷键或工具栏临时切换，切到英文模式时用英文标点；选「始终」时不能临时切换"
      value={punctuationMode(preferences)}
      onChange={(event) => onChange(punctuationModePatch(event.target.value as PunctuationMode))}
    >
      {punctuationModeOptions.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </SelectRow>
  );
}

/** 「智能标点」总开关：用户切换时连同数字后直出、字母后直出一起写（见 `smartPunctuationPatch`）。以前只写总开关的版本留下了「总开关开、两半都关」的文档，这时说明里那句承诺并不成立；这里只按已存的值多显示一句提示，不改写存储。 */
function SmartPunctuationRow({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  const state = punctuationControlState(preferences);
  const inert =
    state.smart_punctuation &&
    !state.smart_punctuation_direct_digit &&
    !state.smart_punctuation_direct_letter;
  return (
    <SwitchRow
      title={smartPunctuation[1]}
      description={
        inert
          ? `${smartPunctuation[2]}。数字后直出和字母后直出都已关闭，这一条暂不生效，打开下面其中一项，或关掉再打开智能标点`
          : smartPunctuation[2]
      }
      checked={state.smart_punctuation}
      onChange={(checked) => onChange(smartPunctuationPatch(checked))}
    />
  );
}

function PunctuationSwitchRow({
  preference,
  title,
  description,
  disabled,
  preferences,
  onChange,
}: {
  preference: PunctuationSwitch;
  title: string;
  description?: string;
  disabled?: boolean;
} & Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  return (
    <SwitchRow
      title={title}
      description={description}
      checked={punctuationControlState(preferences)[preference]}
      disabled={disabled}
      onChange={(checked) =>
        onChange(
          preference === "enumeration_comma_backslash" || preference === "enumeration_comma_slash"
            ? enumerationCommaPatch(preferences, preference, checked)
            : { [preference]: checked },
        )
      }
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

/** 引号样式：只在中文标点下起作用，所以紧跟在「标点」选择之后。直角引号按键对应，说明里写出键位。 */
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

/** HarmonyOS 手机「表达」页的「标点」分组，遵循设计稿和 Android 的 `ExpressionPage`：「使用英文标点」（显示为 `chinese_punctuation` 的反值）和「自动补全成对标点」；引号与方头括号样式、智能标点（默认开，与桌面一样收进折叠区）和它的细分选项、「固定标点」收在「更多选项」下，不丢失任何生效中的设置。 */
export function PhonePunctuationSection({
  preferences,
  onChange,
}: Pick<PunctuationSectionProps, "preferences" | "onChange">) {
  const state = punctuationControlState(preferences);
  const smartEnabled = state.smart_punctuation;
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
      <MoreOptions>
        <QuoteStyleRow preferences={preferences} onChange={onChange} />
        <BracketStyleRow preferences={preferences} onChange={onChange} />
        {enumerationCommaSwitches.map(([key, label, description]) => (
          <PunctuationSwitchRow
            key={key}
            preference={key}
            title={label}
            description={description}
            disabled={
              !state[
                key === "enumeration_comma_backslash"
                  ? "enumeration_comma_slash"
                  : "enumeration_comma_backslash"
              ]
            }
            preferences={preferences}
            onChange={onChange}
          />
        ))}
        <SmartPunctuationRow preferences={preferences} onChange={onChange} />
        {smartPunctuationRefinements.map(([key, label, description]) => (
          <PunctuationSwitchRow
            key={key}
            preference={key}
            title={label}
            description={description}
            disabled={!smartEnabled}
            preferences={preferences}
            onChange={onChange}
          />
        ))}
        <PunctuationLockRow preferences={preferences} onChange={onChange} />
      </MoreOptions>
    </>
  );
}
