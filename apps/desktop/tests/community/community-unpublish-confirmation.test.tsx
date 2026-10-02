// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CommunityUnpublishConfirmation } from "../../../../packages/ui/src/community/community-unpublish-confirmation";

afterEach(cleanup);

test("renders the message and dispatches confirm and cancel actions", () => {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();

  render(
    <CommunityUnpublishConfirmation
      ariaLabel="确认下架作品"
      message={<>确定下架“示例作品”吗？</>}
      actionBusy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  );

  expect(screen.getByRole("alertdialog", { name: "确认下架作品" }).textContent).toContain(
    "确定下架“示例作品”吗？",
  );
  fireEvent.click(screen.getByRole("button", { name: "确认下架" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(onConfirm).toHaveBeenCalledOnce();
  expect(onCancel).toHaveBeenCalledOnce();
});

test("disables both actions while the unpublish request is busy", () => {
  render(
    <CommunityUnpublishConfirmation
      ariaLabel="确认下架插件"
      message="正在下架"
      actionBusy
      onConfirm={vi.fn()}
      onCancel={vi.fn()}
    />,
  );

  expect(screen.getByRole("button", { name: "确认下架" }).hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "取消" }).hasAttribute("disabled")).toBe(true);
});
