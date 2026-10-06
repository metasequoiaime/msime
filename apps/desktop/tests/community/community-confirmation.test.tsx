// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CommunityConfirmationProps = {
  ariaLabel: string;
  message: ReactNode;
  actionBusy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  confirmLabel: ReactNode;
  actionsClassName?: string;
};

test("shared community confirmation keeps message, busy state, and action labels", () => {
  const Confirmation = (
    ui as unknown as {
      CommunityConfirmation: ComponentType<CommunityConfirmationProps>;
    }
  ).CommunityConfirmation;
  expect(Confirmation).toBeDefined();

  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  const { rerender } = render(
    <Confirmation
      ariaLabel="合成确认"
      message="合成提示"
      actionBusy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
      confirmLabel="合成操作"
    />,
  );

  expect(screen.getByRole("alertdialog", { name: "合成确认" }).textContent).toContain("合成提示");
  fireEvent.click(screen.getByRole("button", { name: "合成操作" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(onConfirm).toHaveBeenCalledOnce();
  expect(onCancel).toHaveBeenCalledOnce();

  rerender(
    <Confirmation
      ariaLabel="合成确认"
      message="合成提示"
      actionBusy
      onConfirm={onConfirm}
      onCancel={onCancel}
      confirmLabel="合成操作"
    />,
  );
  expect(screen.getByRole("button", { name: "合成操作" }).hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "取消" }).hasAttribute("disabled")).toBe(true);
});
