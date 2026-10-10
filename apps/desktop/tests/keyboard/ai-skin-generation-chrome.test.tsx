// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AiSkinGeneration } from "../../../../packages/ui/src/keyboard/touch-keyboard-skin-editor";

afterEach(cleanup);

function open(fullScreen: boolean) {
  const onClose = vi.fn();
  render(
    <AiSkinGeneration
      client={{ generate: vi.fn(), poll: vi.fn(), cancel: vi.fn() } as never}
      library={{ list: vi.fn(), save: vi.fn(), remove: vi.fn() } as never}
      fullScreen={fullScreen}
      onUse={vi.fn()}
      onClose={onClose}
    />,
  );
  return onClose;
}

// 手机皮肤网格的 AI 磁贴把抽卡打开成全屏页：标题前的返回就是出口，不再在页尾放一个重复的「完成」；抽卡按钮是这页唯一的主操作。
test("the full-screen draw has one way back and a primary draw button", () => {
  const onClose = open(true);
  expect(screen.queryByRole("button", { name: "完成" })).toBeNull();
  const draw = screen.getByRole("button", { name: "抽三张皮肤" });
  expect(draw.className.split(" ")).toContain("primary");
  expect(draw.className.split(" ")).toContain("w-full");
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(onClose).toHaveBeenCalledOnce();
});

// 全屏视图共用外壳的滚动容器，打开时把自己滚到顶部，标题和返回按钮才在屏幕上。
test("the full-screen draw opens at its top", () => {
  const scrollIntoView = vi.fn();
  const original = HTMLElement.prototype.scrollIntoView;
  HTMLElement.prototype.scrollIntoView = scrollIntoView;
  try {
    open(true);
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "start" });
    cleanup();
    scrollIntoView.mockClear();
    open(false);
    expect(scrollIntoView).not.toHaveBeenCalled();
  } finally {
    HTMLElement.prototype.scrollIntoView = original;
  }
});

test("the dialog draw keeps its 完成 button", () => {
  const onClose = open(false);
  fireEvent.click(screen.getByRole("button", { name: "完成" }));
  expect(onClose).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "抽三张皮肤" }).className).toBe("primary");
});
