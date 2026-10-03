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
  /** 本版本提供的方案（`HostCapabilities.edition.input_schemes`）。本版本没有的输入模式直接不列出；只剩「中文」时整行隐藏。缺省（full）列出全部模式。 */
  editionSchemes?: readonly InputScheme[];
  /** 本版本的默认方案，回到中文时记住的中文方案也不可用就用它。缺省是全拼。 */
  defaultScheme?: InputModeChineseScheme;
  /** Hosts that choose among touch keyboard schemes instead keep this row out of sight. */
  hidden?: boolean;
}

type InputMode = "chinese" | (typeof nonChineseSchemes)[number];

const inputModeLabels: Record<InputMode, string> = {
  chinese: "中文",
  japanese: "日文",
  korean: "韩文",
  vietnamese: "越南文",
};

/** 把模式名连成「中文、日文、韩文或越南文」。 */
function joinModes(modes: readonly InputMode[]): string {
  const labels = modes.map((mode) => inputModeLabels[mode]);
  return labels.length > 1
    ? `${labels.slice(0, -1).join("、")}或${labels[labels.length - 1]}`
    : (labels[0] ?? "");
}

/** Chinese/Japanese/Korean/Vietnamese input mode selector with remembered Chinese scheme: the first row of the 方案 group. */
export function InputModeSection({
  scheme,
  lastChineseScheme,
  onChange,
  supportedSchemes = baseInputSchemes,
  editionSchemes,
  defaultScheme = "quanpin",
  hidden,
}: InputModeSectionProps) {
  // 本版本没有的模式不列出，而不是显示为禁用。
  const modes: readonly InputMode[] = [
    "chinese",
    ...nonChineseSchemes.filter((mode) => !editionSchemes || editionSchemes.includes(mode)),
  ];
  const unsupported = nonChineseSchemes.filter(
    (mode) => modes.includes(mode) && !supportedSchemes.includes(mode),
  );
  const options = modes.map((mode) => ({
    value: mode,
    label: inputModeLabels[mode],
    disabled: mode !== "chinese" && unsupported.includes(mode),
  }));
  const chineseFallback = fallbackChineseScheme(lastChineseScheme, supportedSchemes, defaultScheme);
  const base = `切换${joinModes(modes)}输入，并保留各模式上次选择的方案`;
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
      hidden={hidden || modes.length <= 1}
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
