// @vitest-environment jsdom
import type { ComponentType } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CommunityDialogHeaderProps = {
  title: string;
  busy?: boolean;
  onClose: () => void;
};

test("shared community dialog header renders its title and guarded close button", () => {
  const Header = (
    ui as unknown as {
      CommunityDialogHeader: ComponentType<CommunityDialogHeaderProps>;
    }
  ).CommunityDialogHeader;
  expect(Header).toBeDefined();

  const onClose = vi.fn();
  const { rerender } = render(<Header title="合成标题" onClose={onClose} />);
  expect(screen.getByRole("heading", { name: "合成标题" })).not.toBeNull();

  fireEvent.click(screen.getByRole("button", { name: "关闭发布窗口" }));
  expect(onClose).toHaveBeenCalledOnce();

  rerender(<Header title="合成标题" busy onClose={onClose} />);
  expect(screen.getByRole("button", { name: "关闭发布窗口" })).toHaveProperty("disabled", true);
});
