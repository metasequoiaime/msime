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
  /** The remembered Chinese scheme, which host-api runs in place of an unsupported `value`. */
  lastChineseScheme?: ChineseInputScheme | null;
  hidden?: boolean;
}

/** Why some schemes are disabled, and which scheme runs when the selected one is among them; undefined when every scheme is offered. */
function supportHint(
  value: InputSchemeSelectorValue,
  supported: readonly InputScheme[],
  lastChineseScheme: ChineseInputScheme | null | undefined,
): string | undefined {
  const unsupported = chineseInputSchemeOptions.filter(({ value }) => !supported.includes(value));
  if (unsupported.length === 0) return undefined;
  const note = `此平台暂不支持${unsupported.map(({ label }) => label).join("、")}`;
  return supported.includes(value)
    ? note
    : `${note}，已回退到${schemeTitle(fallbackChineseScheme(lastChineseScheme, supported))}`;
}

/** Radio selector for the desktop Chinese input schemes. */
export function InputSchemeSelectorSection({
  value,
  onChange,
  supportedSchemes = baseInputSchemes,
  lastChineseScheme,
  hidden,
}: InputSchemeSelectorSectionProps) {
  const options = chineseInputSchemeOptions.map((option) => ({
    ...option,
    disabled: !supportedSchemes.includes(option.value),
  }));
  const hint = supportHint(value, supportedSchemes, lastChineseScheme);
  // Five two-character segments come to about 262px. Beside the macOS look's 260px sidebar and 48px page margins the row fits in any window from about 675px wide, and the window opens at 1000px, so this stays a Segmented rather than falling back to a Select.
  return (
    <SegmentedRow
      title="输入方案"
      description={hint}
      hidden={hidden}
      options={options}
      value={value}
      onChange={onChange}
    />
  );
}
