// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CommunityDetailFrame } from "@msime/ui";

afterEach(() => cleanup());

test("renders the shared community detail shell and forwards navigation actions", () => {
  const onBack = vi.fn();
  const onLogin = vi.fn();
  render(
    <CommunityDetailFrame
      backDisabled={false}
      onBack={onBack}
      error="需要登录"
      signInRequired
      onLogin={onLogin}
    >
      <p>详情内容</p>
    </CommunityDetailFrame>,
  );

  expect(screen.getByText("详情内容")).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toContain("需要登录");
  fireEvent.click(screen.getByRole("button", { name: "返回社区" }));
  fireEvent.click(screen.getByRole("button", { name: "去登录" }));
  expect(onBack).toHaveBeenCalledOnce();
  expect(onLogin).toHaveBeenCalledOnce();
});

test("supports a detail-specific back button accessible name", () => {
  render(
    <CommunityDetailFrame backDisabled={false} onBack={() => undefined} backAriaLabel="← 社区">
      <p>资源详情</p>
    </CommunityDetailFrame>,
  );

  expect(screen.getByRole("button", { name: "← 社区" })).toBeTruthy();
});

test("plugin gallery reuses the shared community detail frame", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-plugins.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain('import { CommunityDetailFrame } from "./community-detail-frame";');
  expect(source).toContain("<CommunityDetailFrame");
  expect(source).not.toContain(
    "<CommunityBackButton disabled={actionBusy} onClick={closeDetail} />",
  );
  expect(source).not.toContain("<section className={`section ${style.detail}`}>");
});

test("resource gallery reuses the shared community detail frame", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/community/community-resources.tsx", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain('import { CommunityDetailFrame } from "./community-detail-frame";');
  expect(source).toContain("<CommunityDetailFrame");
  expect(source).not.toContain(
    '<CommunityBackButton ariaLabel="← 社区" disabled={busy} onClick={close} />',
  );
  expect(source).not.toContain("<section className={`section ${style.detail}`}>");
});
