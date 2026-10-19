import type { InputScheme } from "../core/input-scheme";

import { useId } from "react";
import { SettingsManagerBlock } from "./settings-manager-block";
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

/** 桌面中文方案单独占一块网格，与上方的语言下拉区分层级。 */
export function InputSchemeSelectorSection({
  value,
  onChange,
  supportedSchemes = baseInputSchemes,
  editionSchemes,
  defaultScheme = "quanpin",
  lastChineseScheme,
  hidden,
}: InputSchemeSelectorSectionProps) {
  const id = useId();
  const listed = editionSchemes
    ? chineseInputSchemeOptions.filter((option) => editionSchemes.includes(option.value))
    : chineseInputSchemeOptions;
  const options = listed.map((option) => ({
    ...option,
    disabled: !supportedSchemes.includes(option.value),
  }));
  const hint = supportHint(value, listed, supportedSchemes, lastChineseScheme, defaultScheme);
  return (
    <SettingsManagerBlock hidden={hidden || options.length <= 1}>
      <span id={`${id}-title`} data-row-title="" className="[font-size:var(--p-row-fs)]">
        中文方案
      </span>
      <div
        role="radiogroup"
        aria-labelledby={`${id}-title`}
        aria-describedby={hint ? `${id}-hint` : undefined}
        className="grid grid-cols-3 gap-2"
      >
        {options.map((option) => (
          <label
            key={option.value}
            className="flex min-w-0 cursor-pointer items-center gap-2.5 rounded-lg border [border-color:var(--p-row-divider)] px-3 py-3 [font-size:var(--p-row-fs)] hover:bg-[var(--p-hover)] has-[:checked]:border-accent has-[:checked]:bg-[var(--accent-soft)] has-[:disabled]:cursor-default has-[:disabled]:opacity-50 has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-2 has-[:focus-visible]:outline-accent"
          >
            <input
              type="radio"
              name={id}
              value={option.value}
              checked={value === option.value}
              disabled={option.disabled}
              onChange={() => onChange(option.value)}
              className="m-0 shrink-0 accent-accent"
            />
            {option.label}
          </label>
        ))}
      </div>
      {hint && (
        <span id={`${id}-hint`} className="[font-size:var(--p-sub-fs)] [color:var(--p-sub)]">
          {hint}
        </span>
      )}
    </SettingsManagerBlock>
  );
}
