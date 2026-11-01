import { useId, useLayoutEffect, type ReactNode } from "react";
import { textOf, useGroupNotes } from "../core/platform-controls";
import * as settings from "./settings-style";

export interface SettingsGroupNoteProps {
  children: ReactNode;
  className?: string;
  role?: "status";
  /** 这段是组的内容而不是解释，始终画在原处：空列表提示、预览截断提示、错误和风险警告、当前状态，以及「当前宿主/桌面下这组设置不起作用」这类宿主限制。 */
  inline?: boolean;
}

/**
 * Shared explanatory paragraph used inside settings groups.
 *
 * 设计语言（见 docs/ARCHITECTURE.md「设置项说明收进「?」」）：在有组名的 `GroupList` 里，解释性说明不常驻页面，交给组名旁的「?」在气泡里显示（见 `useGroupNotes`）。`role="status"` 的反馈和标了 `inline` 的内容始终画在原处；组外、无组名的组和 `GroupNotesInline` 里面也画在原处。
 */
export function SettingsGroupNote({ children, className, role, inline }: SettingsGroupNoteProps) {
  const group = useGroupNotes();
  const id = useId();
  const collected = group !== null && role !== "status" && !inline;
  // 按文字判断内容是否变了：说明里常夹着地址之类的值，每次渲染都是新的节点，直接拿节点比较会让组反复重渲染。
  const text = textOf(children);
  useLayoutEffect(() => {
    if (!collected) return;
    return group.add(id, children);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `children` is tracked through `text`.
  }, [collected, group, id, text]);
  if (collected) return null;
  return (
    <p className={`${settings.groupNote}${className ? ` ${className}` : ""}`} role={role}>
      {children}
    </p>
  );
}
