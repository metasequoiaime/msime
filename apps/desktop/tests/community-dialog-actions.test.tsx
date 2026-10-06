// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type DialogActionsProps = {
  busy?: boolean;
  onClose: () => void;
  children: ReactNode;
};

test("community dialog actions shares cancel behavior and keeps custom actions", () => {
  const Actions = (ui as unknown as { CommunityDialogActions: ComponentType<DialogActionsProps> })
    .CommunityDialogActions;
  expect(Actions).toBeDefined();

  const onClose = vi.fn();
  render(
    <Actions busy={false} onClose={onClose}>
      <button type="submit">公开发布</button>
    </Actions>,
  );

  expect(screen.getByRole("button", { name: "公开发布" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(onClose).toHaveBeenCalledOnce();
});

test("community dialog actions disables cancel while busy", () => {
  const Actions = (ui as unknown as { CommunityDialogActions: ComponentType<DialogActionsProps> })
    .CommunityDialogActions;
  render(
    <Actions busy onClose={vi.fn()}>
      <button type="submit">继续</button>
    </Actions>,
  );

  expect(screen.getByRole("button", { name: "取消" })).toHaveProperty("disabled", true);
});
