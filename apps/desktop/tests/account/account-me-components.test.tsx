// @vitest-environment jsdom
import type { ComponentType, ReactNode } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type MeGroupProps = { title?: string; children: ReactNode };
type MeRowProps = { title: string; disabled?: boolean; onClick: () => void };
type SupportProps = {
  disabled?: boolean;
  onReplayOnboarding?: () => void;
  onOpenFeedback?: () => void;
  onOpenAbout?: () => void;
};

test("account me group labels titled and untitled rows", () => {
  const Group = (ui as unknown as { AccountMeGroup: ComponentType<MeGroupProps> }).AccountMeGroup;
  expect(Group).toBeDefined();
  const { rerender } = render(
    <Group title="通用">
      <span>隐私</span>
    </Group>,
  );
  const titled = screen.getByRole("region", { name: "通用" });
  expect(titled.textContent).toContain("隐私");

  rerender(
    <Group>
      <span>关于</span>
    </Group>,
  );
  expect(screen.queryByRole("heading")).toBeNull();
  expect(screen.getByText("关于")).toBeTruthy();
});

test("account me section shares the group shell without imposing a row container", () => {
  const Section = (ui as unknown as { AccountMeSection: ComponentType<MeGroupProps> })
    .AccountMeSection;
  expect(Section).toBeDefined();
  render(
    <Section title="下载">
      <div data-testid="download-rows">电脑</div>
    </Section>,
  );

  const section = screen.getByRole("region", { name: "下载" });
  expect(section.contains(screen.getByTestId("download-rows"))).toBe(true);
  expect(section.querySelector('[role="group"]')).toBeNull();
});

test("account me row keeps disabled state and invokes its action", () => {
  const Row = (ui as unknown as { AccountMeRow: ComponentType<MeRowProps> }).AccountMeRow;
  expect(Row).toBeDefined();
  const onClick = vi.fn();
  render(<Row title="云词库" onClick={onClick} />);
  const row = screen.getByRole("button", { name: "云词库" });
  fireEvent.click(row);
  expect(onClick).toHaveBeenCalledOnce();
  expect(row.textContent).toContain("›");

  render(<Row title="云剪贴板" disabled onClick={onClick} />);
  const disabled = screen.getByRole("button", { name: "云剪贴板" });
  expect(disabled).toHaveProperty("disabled", true);
});

test("account me support group omits itself without actions and renders supplied actions", () => {
  const Support = (ui as unknown as { AccountMeSupportGroup: ComponentType<SupportProps> })
    .AccountMeSupportGroup;
  expect(Support).toBeDefined();
  const { rerender } = render(<Support />);
  expect(screen.queryByRole("button")).toBeNull();

  const onReplayOnboarding = vi.fn();
  const onOpenFeedback = vi.fn();
  rerender(<Support onReplayOnboarding={onReplayOnboarding} onOpenFeedback={onOpenFeedback} />);
  fireEvent.click(screen.getByRole("button", { name: "新手引导" }));
  fireEvent.click(screen.getByRole("button", { name: "帮助与反馈" }));
  expect(onReplayOnboarding).toHaveBeenCalledOnce();
  expect(onOpenFeedback).toHaveBeenCalledOnce();
});
