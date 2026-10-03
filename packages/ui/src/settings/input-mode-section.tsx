import type { ChineseScheme, InputScheme } from "../index";
import { SegmentedRow } from "./segmented-row";
import {
  baseInputSchemes,
  fallbackChineseScheme,
  isChineseScheme,
  nonChineseSchemes,
} from "./input-scheme-options";
import { schemeTitle } from "./label-helpers";

export type InputModeScheme = InputScheme;
export type InputModeChineseScheme = ChineseScheme;

export interface InputModeSectionProps {
  scheme: InputModeScheme;
  lastChineseScheme?: InputModeChineseScheme | null;
  onChange: (
    patch: Partial<{
      scheme: InputModeScheme;
      last_chinese_scheme: InputModeChineseScheme | null;
    }>,
  ) => void;
  /** The schemes the host offers (`HostCapabilities.input_schemes`); a mode outside it is shown disabled. Defaults to the five every host offers. */
  supportedSchemes?: readonly InputScheme[];
  /** Hosts that choose among touch keyboard schemes instead keep this row out of sight. */
  hidden?: boolean;
}

type InputMode = "chinese" | (typeof nonChineseSchemes)[number];

const inputModeLabels: Record<InputMode, string> = {
  chinese: "中文",
  japanese: "日文",
  korean: "韩文",
  vietnamese: "越南文",
  tibetan: "藏文",
};

/** 中文、日文、韩文、越南文、藏文输入模式选择器，记住上次的中文方案：「方案」分组的第一行。 */
export function InputModeSection({
  scheme,
  lastChineseScheme,
  onChange,
  supportedSchemes = baseInputSchemes,
  hidden,
}: InputModeSectionProps) {
  const unsupported = nonChineseSchemes.filter((mode) => !supportedSchemes.includes(mode));
  const options = (["chinese", ...nonChineseSchemes] as const).map((mode) => ({
    value: mode,
    label: inputModeLabels[mode],
    disabled: mode !== "chinese" && unsupported.includes(mode),
  }));
  const chineseFallback = fallbackChineseScheme(lastChineseScheme, supportedSchemes);
  const base = "切换中文、日文、韩文、越南文或藏文输入，并保留各模式上次选择的方案";
  // A document naming a mode this host does not offer runs host-api's fallback scheme, and the row says which.
  const description =
    !isChineseScheme(scheme) && unsupported.includes(scheme)
      ? `此平台暂不支持${inputModeLabels[scheme]}，已回退到${schemeTitle(chineseFallback)}`
      : unsupported.length > 0
        ? `${base}；此平台暂不支持${unsupported.map((mode) => inputModeLabels[mode]).join("、")}`
        : base;
  return (
    <SegmentedRow
      title="输入模式"
      description={description}
      hidden={hidden}
      options={options}
      value={isChineseScheme(scheme) ? "chinese" : scheme}
      onChange={(mode) =>
        onChange(
          mode === "chinese"
            ? { scheme: chineseFallback }
            : {
                last_chinese_scheme: isChineseScheme(scheme) ? scheme : lastChineseScheme,
                scheme: mode,
              },
        )
      }
    />
  );
}
