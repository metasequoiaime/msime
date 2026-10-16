import type { InputScheme } from "../core/input-scheme";

import { SegmentedRow } from "./segmented-row";
import {
  chineseOutputForm,
  chineseOutputPatch,
  type ChineseOutputForm,
  type TraditionalChineseOutputValue,
  type TraditionalChineseVariant,
} from "./chinese-output-preferences";

export type {
  ChineseOutputForm,
  TraditionalChineseOutputValue,
  TraditionalChineseVariant,
} from "./chinese-output-preferences";

export interface TraditionalChineseOutputSectionProps {
  /** 偏好 `traditional_chinese_output`。 */
  value?: boolean;
  /** 偏好 `traditional_chinese_variant`；缺省为 `s2t`。 */
  variant?: TraditionalChineseVariant;
  /** 宿主能按变体转换时列出台湾的两项（`HostCapabilities.traditional_chinese_variants`），否则只有简体和繁体。 */
  variants?: boolean;
  /** 文档的输入方案；粤拼、注音直接输出繁体，笔画按所选字原样输出，行内说明该选项不影响它们。 */
  scheme?: InputScheme;
  /** 只交回变了的偏好：选简体时变体不动，下次选回繁体还是它；宿主不认变体时也不动它。 */
  onChange: (patch: Partial<TraditionalChineseOutputValue>) => void;
}

const standardOptions = [
  { value: "simplified", label: "简体" },
  { value: "s2t", label: "繁体" },
] as const satisfies readonly { value: ChineseOutputForm; label: string }[];

const variantOptions = [
  ...standardOptions,
  { value: "s2tw", label: "台湾正体" },
  { value: "s2twp", label: "台湾用词" },
] as const satisfies readonly { value: ChineseOutputForm; label: string }[];

const formDescriptions: Record<ChineseOutputForm, string> = {
  simplified: "按简体中文提交",
  s2t: "将提交的简体中文转换为繁体中文（OpenCC 标准繁体）",
  s2tw: "将提交的简体中文转换为台湾正体字形，如「裡面」「著急」（OpenCC s2tw）",
  s2twp: "转换为台湾正体字形，并换用台湾惯用词，如「軟體」「滑鼠」（OpenCC s2twp）",
};

/** 「中文输出」单选：简体、繁体，宿主支持时再加台湾正体和台湾用词。输入页「输出」组的一行。 */
export function TraditionalChineseOutputSection({
  value,
  variant,
  variants,
  scheme,
  onChange,
}: TraditionalChineseOutputSectionProps) {
  const form = chineseOutputForm(value, variant, variants);
  const native = scheme === "cantonese" || scheme === "zhuyin";
  const note = native
    ? "。粤拼与注音直接输出繁体，此选项不影响它们"
    : scheme === "stroke"
      ? "。笔画按所选的字原样输出，此选项不影响它"
      : "";
  return (
    <SegmentedRow<ChineseOutputForm>
      title="中文输出"
      description={formDescriptions[form] + note}
      options={variants ? variantOptions : standardOptions}
      value={form}
      onChange={(next) => onChange(chineseOutputPatch(next, variant, variants))}
    />
  );
}
