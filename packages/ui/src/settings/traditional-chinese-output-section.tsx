import type { InputScheme } from "../index";
import { SwitchRow } from "./switch-row";

export interface TraditionalChineseOutputSectionProps {
  value?: boolean;
  /** The document's input scheme; Cantonese and Zhuyin commit Traditional characters as stored and Stroke commits the character picked as it is, so the switch does not touch them and the row says so. */
  scheme?: InputScheme;
  onChange: (value: boolean) => void;
}

/** Simplified-to-traditional output switch shared by input settings hosts: one row of the 输出 group. */
export function TraditionalChineseOutputSection({
  value,
  scheme,
  onChange,
}: TraditionalChineseOutputSectionProps) {
  const native = scheme === "cantonese" || scheme === "zhuyin";
  return (
    <SwitchRow
      title="繁体输出"
      description={
        native
          ? "将提交的简体中文转换为繁体中文。粤拼与注音直接输出繁体，此开关不影响它们"
          : scheme === "stroke"
            ? "将提交的简体中文转换为繁体中文。笔画按所选的字原样输出，此开关不影响它"
            : "将提交的简体中文转换为繁体中文"
      }
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
