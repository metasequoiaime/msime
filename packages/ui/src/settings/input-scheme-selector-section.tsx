import type { InputScheme } from "../index";
import { SegmentedRow } from "./segmented-row";
import {
  baseInputSchemes,
  chineseInputSchemeOptions,
  fallbackChineseScheme,
  type ChineseInputScheme,
} from "./input-scheme-options";
import { schemeTitle } from "./label-helpers";

export type InputSchemeSelectorValue = ChineseInputScheme;

export interface InputSchemeSelectorSectionProps {
  value: InputSchemeSelectorValue;
  onChange: (value: InputSchemeSelectorValue) => void;
  /** The schemes the host offers (`HostCapabilities.input_schemes`); a scheme outside it is shown disabled. Defaults to the five every host offers. */
  supportedSchemes?: readonly InputScheme[];
  /** 本版本提供的方案（`HostCapabilities.edition.input_schemes`）。不在其中的方案在本版本里不存在，直接不列出；只剩一个方案时整行隐藏。缺省（full）列出全部方案。 */
  editionSchemes?: readonly InputScheme[];
  /** 本版本的默认方案，记住的中文方案也不可用时 host-api 回退到它。缺省是全拼。 */
  defaultScheme?: ChineseInputScheme;
  /** The remembered Chinese scheme, which host-api runs in place of an unsupported `value`. */
  lastChineseScheme?: ChineseInputScheme | null;
  hidden?: boolean;
}

/** 说明哪些方案被禁用、当前方案不可用时实际运行哪个方案；每个列出的方案都可用、当前方案也可用时为 undefined。 */
function supportHint(
  value: InputSchemeSelectorValue,
  options: readonly { value: ChineseInputScheme; label: string }[],
  supported: readonly InputScheme[],
  lastChineseScheme: ChineseInputScheme | null | undefined,
  defaultScheme: ChineseInputScheme,
): string | undefined {
  const unsupported = options.filter(({ value }) => !supported.includes(value));
  const note =
    unsupported.length > 0
      ? `此平台暂不支持${unsupported.map(({ label }) => label).join("、")}`
      : undefined;
  if (supported.includes(value)) return note;
  const fallback = `已回退到${schemeTitle(fallbackChineseScheme(lastChineseScheme, supported, defaultScheme))}`;
  return note ? `${note}，${fallback}` : fallback;
}

/** Radio selector for the desktop Chinese input schemes. */
export function InputSchemeSelectorSection({
  value,
  onChange,
  supportedSchemes = baseInputSchemes,
  editionSchemes,
  defaultScheme = "quanpin",
  lastChineseScheme,
  hidden,
}: InputSchemeSelectorSectionProps) {
  const listed = editionSchemes
    ? chineseInputSchemeOptions.filter((option) => editionSchemes.includes(option.value))
    : chineseInputSchemeOptions;
  const options = listed.map((option) => ({
    ...option,
    disabled: !supportedSchemes.includes(option.value),
  }));
  const hint = supportHint(value, listed, supportedSchemes, lastChineseScheme, defaultScheme);
  // Five two-character segments measured about 262px, so the six with 笔画 come to about 315px. Beside the macOS look's 260px sidebar and 48px page margins the row fits in any window from about 730px wide, and the window opens at 1000px, so this stays a Segmented rather than falling back to a Select.
  return (
    <SegmentedRow
      title="输入方案"
      description={hint}
      hidden={hidden || options.length <= 1}
      options={options}
      value={value}
      onChange={onChange}
    />
  );
}
