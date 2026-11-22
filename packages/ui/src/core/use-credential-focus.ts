import { useLayoutEffect, useRef } from "react";
import { subscribeToDocumentVisibilityChange, subscribeToWindowBlur } from "./window-focus";

function releasePasswordFocus(input: HTMLInputElement | null) {
  if (
    input &&
    input.ownerDocument.activeElement === input &&
    input.closest('[data-settings-shell][data-platform="mac"]')
  ) {
    input.blur();
  }
}

/** 在 macOS 设置窗口退出凭据编辑时让 WKWebView 完成密码输入的失焦。 */
export function useCredentialFocus(masked: boolean) {
  const inputRef = useRef<HTMLInputElement>(null);

  useLayoutEffect(() => {
    const input = inputRef.current;
    if (!masked || !input) return;
    const release = () => releasePasswordFocus(input);
    const stopBlur = subscribeToWindowBlur(release);
    const stopVisibility = subscribeToDocumentVisibilityChange(() => {
      if (input.ownerDocument.visibilityState === "hidden") release();
    });
    return () => {
      stopBlur();
      stopVisibility();
      release();
    };
  }, [masked]);

  const release = () => {
    if (masked) releasePasswordFocus(inputRef.current);
  };
  return { inputRef, release };
}
