import type { ReactNode } from "react";
import { InfoTip } from "../core/info-tip";

export interface SettingSectionTitleProps {
  title: ReactNode;
  description?: ReactNode;
  as?: "div" | "p" | "span";
  className?: string;
}

/** Shared title markup for the standalone settings field and section layouts. 说明收进标题旁悬停可见的「?」：这个标题常在 `<label>` 里（`SettingField`），那里不能放按钮，说明仍以文字留在标签里，由输入框的名称读出。 */
export function SettingSectionTitle({
  title,
  description,
  as = "span",
  className = "section-title",
}: SettingSectionTitleProps) {
  const Title = as;
  return (
    <Title className={className}>
      {title}
      {description !== undefined && (
        <InfoTip passive className="ml-1">
          {description}
        </InfoTip>
      )}
    </Title>
  );
}
