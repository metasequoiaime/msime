// @vitest-environment jsdom
import type { ComponentType, FormEvent, KeyboardEvent, ReactNode } from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CommunityDialogHeaderProps = {
  title: string;
  busy?: boolean;
  onClose: () => void;
};

type CommunityDialogFrameProps = {
  title: string;
  ariaLabel: string;
  busy?: boolean;
  onClose: () => void;
  error?: string;
  signInRequired?: boolean;
  onLogin?: () => void;
  titleClassName?: string;
  children: ReactNode;
  onSubmit?: (event: FormEvent<HTMLFormElement>) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLElement>) => void;
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

test("shared community dialog frame keeps form semantics and error actions", () => {
  const Frame = (
    ui as unknown as {
      CommunityDialogFrame: ComponentType<CommunityDialogFrameProps>;
    }
  ).CommunityDialogFrame;
  expect(Frame).toBeDefined();

  const onClose = vi.fn();
  const onLogin = vi.fn();
  const onSubmit = vi.fn((event: FormEvent<HTMLFormElement>) => event.preventDefault());
  render(
    <Frame
      title="合成发布"
      ariaLabel="合成发布对话框"
      onClose={onClose}
      error="合成错误"
      signInRequired
      onLogin={onLogin}
      onSubmit={onSubmit}
    >
      <p>合成内容</p>
    </Frame>,
  );

  const dialog = screen.getByRole("dialog", { name: "合成发布对话框" });
  expect(dialog.tagName).toBe("FORM");
  expect(screen.getByRole("heading", { name: "合成发布" })).not.toBeNull();
  expect(screen.getByRole("alert").textContent).toContain("合成错误");
  expect(screen.getByText("合成内容")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "去登录" }));
  expect(onLogin).toHaveBeenCalledOnce();
  fireEvent.submit(dialog);
  expect(onSubmit).toHaveBeenCalledOnce();
});
