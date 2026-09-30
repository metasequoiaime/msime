// @vitest-environment jsdom
import type { ComponentType } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import * as ui from "@msime/ui";

afterEach(cleanup);

type CommunityScopeButtonsProps = {
  ariaLabel: string;
  mineOnly: boolean;
  allLabel: string;
  mineLabel: string;
  onMineOnlyChange: (mineOnly: boolean) => void;
};

test("community scope buttons expose the active scope and forward changes", () => {
  const ScopeButtons = (
    ui as unknown as { CommunityScopeButtons: ComponentType<CommunityScopeButtonsProps> }
  ).CommunityScopeButtons;
  expect(ScopeButtons).toBeDefined();

  const onMineOnlyChange = vi.fn();
  render(
    <ScopeButtons
      ariaLabel="社区皮肤范围"
      mineOnly
      allLabel="全部皮肤"
      mineLabel="我的作品"
      onMineOnlyChange={onMineOnlyChange}
    />,
  );

  expect(screen.getByRole("group", { name: "社区皮肤范围" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "我的作品" }).getAttribute("aria-pressed")).toBe(
    "true",
  );
  fireEvent.click(screen.getByRole("button", { name: "全部皮肤" }));
  expect(onMineOnlyChange).toHaveBeenCalledWith(false);
});
