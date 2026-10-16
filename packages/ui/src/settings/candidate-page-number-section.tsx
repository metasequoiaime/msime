import type { HostPlatform } from "../core/host-capabilities";

import { SwitchRow } from "./switch-row";

export interface CandidatePageNumberSectionProps {
  value?: boolean;
  /** macOS 的开关连同首行的 ‹ › 一起隐藏，标题和说明要讲清这一点；其他宿主只藏页码文字。 */
  platform?: HostPlatform;
  onChange: (value: boolean) => void;
}

/** 「显示页码」开关，供会绘制页码的宿主使用（`HostCapabilities::candidate_page_number`）：「候选窗口」页「更多选项」里的一行。 */
export function CandidatePageNumberSection({
  value,
  platform,
  onChange,
}: CandidatePageNumberSectionProps) {
  const withArrows = platform === "macos";
  return (
    <SwitchRow
      title={withArrows ? "显示页码和翻页按钮" : "显示页码"}
      description={
        withArrows
          ? "关闭后候选窗不再显示「1 / 3」和 ‹ ›，仍可用键盘翻页；鼠标滚轮翻页默认关闭，需要时在「输入 › 选词与翻页」里打开。"
          : "显示候选列表的当前页与总页数；关闭后仍可正常翻页。"
      }
      checked={value !== false}
      onChange={onChange}
    />
  );
}
