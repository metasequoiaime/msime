// @vitest-environment jsdom
import { StrictMode, useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test } from "vitest";
import { BottomSheet } from "../../../../packages/ui/src/account/login-sheet";

afterEach(cleanup);

function Harness() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        打开账号面板
      </button>
      <input aria-label="页面输入框" />
      <button type="button" onClick={() => setOpen(false)}>
        从页面关闭面板
      </button>
      {open && (
        <BottomSheet title="合成账号面板" onClose={() => setOpen(false)}>
          <input aria-label="合成昵称" data-sheet-autofocus />
        </BottomSheet>
      )}
    </>
  );
}

function openSheet(strict: boolean) {
  render(
    strict ? (
      <StrictMode>
        <Harness />
      </StrictMode>
    ) : (
      <Harness />
    ),
  );
  const opener = screen.getByRole("button", { name: "打开账号面板" });
  opener.focus();
  fireEvent.click(opener);
  const field = screen.getByRole("textbox", { name: "合成昵称" });
  expect(document.activeElement).toBe(field);
  return { opener, field };
}

test.each(
  [false, true].flatMap((strict) => ["关闭按钮", "Escape"].map((action) => ({ strict, action }))),
)("账号面板 $action 关闭后恢复开启按钮焦点（StrictMode=$strict）", ({ strict, action }) => {
  const { opener, field } = openSheet(strict);
  if (action === "关闭按钮") {
    const close = screen.getByRole("button", { name: "关闭" });
    close.focus();
    fireEvent.click(close);
  } else {
    fireEvent.keyDown(field, { key: "Escape" });
  }
  expect(screen.queryByRole("dialog", { name: "合成账号面板" })).toBeNull();
  expect(document.activeElement).toBe(opener);
});

test.each([false, true])("账号面板保留已移到页面上的焦点（StrictMode=%s）", (strict) => {
  openSheet(strict);
  const outside = screen.getByRole("textbox", { name: "页面输入框" });
  outside.focus();
  fireEvent.click(screen.getByRole("button", { name: "从页面关闭面板" }));
  expect(screen.queryByRole("dialog", { name: "合成账号面板" })).toBeNull();
  expect(document.activeElement).toBe(outside);
});
