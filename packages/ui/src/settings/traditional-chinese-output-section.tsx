import type { InputScheme } from "../index";
import { SwitchRow } from "./switch-row";

export interface TraditionalChineseOutputSectionProps {
  value?: boolean;
  /** The document's input scheme; Cantonese and Zhuyin commit Traditional characters as stored, so the switch does not touch them and the row says so. */
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
      title="简繁输入"
      description={
        native
          ? "将提交的简体中文转换为繁体中文。粤拼与注音直接输出繁体，此开关不影响它们"
          : "将提交的简体中文转换为繁体中文"
      }
      checked={value ?? false}
      onChange={onChange}
    />
  );
}
