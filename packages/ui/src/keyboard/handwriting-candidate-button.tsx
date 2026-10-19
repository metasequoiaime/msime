import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { isComposingKeyEvent } from "../core/ime-key-event";
import { subscribeToElementResize } from "../core/element-resize";
import { subscribeToWindowEvent } from "../core/window-focus";
import { clamp } from "../core/number";
import * as surface from "./panel-surface-style";

export interface HandwritingCandidateButtonProps {
  candidate: string;
  copy: boolean;
  disabled: boolean;
  onChoose: () => void;
  onCopy?: () => void;
}

/** 手写候选按钮：按元素宽度调整字号，并在支持时提供复制快捷键。 */
export function HandwritingCandidateButton({
  candidate,
  copy,
  disabled,
  onChoose,
  onCopy,
}: HandwritingCandidateButtonProps) {
  const button = useRef<HTMLButtonElement>(null);
  const [width, setWidth] = useState(64);
  useEffect(() => {
    const element = button.current;
    if (!element) return;
    const resize = () => setWidth(element.getBoundingClientRect().width);
    resize();
    return subscribeToElementResize(element, resize) ?? subscribeToWindowEvent("resize", resize);
  }, []);
  // 使用 Unicode 码点计数，避免补充平面汉字被当成两个字形。
  const length = Math.max(1, Array.from(candidate).length);
  const fontSize = clamp(Math.min(width * 0.52, (width - 12) / length), 13, 34);
  function copyShortcut(event: ReactKeyboardEvent<HTMLButtonElement>) {
    if (
      !onCopy ||
      disabled ||
      event.defaultPrevented ||
      isComposingKeyEvent(event) ||
      !event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey ||
      event.key.toLowerCase() !== "c"
    )
      return;
    event.preventDefault();
    if (!event.repeat) onCopy();
  }
  return (
    <button
      ref={button}
      type="button"
      className={surface.candidateSubmit}
      style={{ fontSize }}
      title={`${copy ? "复制" : "输入"}：${candidate}`}
      aria-keyshortcuts={onCopy ? "Control+c" : undefined}
      disabled={disabled}
      onClick={onChoose}
      onKeyDown={copyShortcut}
    >
      {candidate}
    </button>
  );
}
