// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LearningDataSection } from "@msime/ui";
import { utilityCss } from "../support/utility-css";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("learning data section reports a reset request", () => {
  const onReset = vi.fn();
  render(<LearningDataSection onReset={onReset} disabled={false} />);

  expect(screen.getByRole("region", { name: "学习数据" })).toBeTruthy();
  expect(
    screen.getByText(
      "清除候选词频、用户词库和拼音学习记录；自己新增和修改的词条也会删除，输入方案与其他设置不会改变。",
    ),
  ).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "清除全部学习数据" }));
  expect(onReset).toHaveBeenCalledTimes(1);
});

test("learning data section disables the reset button while busy", () => {
  render(<LearningDataSection onReset={vi.fn()} disabled />);

  expect(
    (screen.getByRole("button", { name: "清除全部学习数据" }) as HTMLButtonElement).disabled,
  ).toBe(true);
});

test("learning data section draws its reset as the shared danger button", () => {
  render(<LearningDataSection onReset={vi.fn()} disabled={false} />);

  expect(screen.getByRole("button", { name: "清除全部学习数据" }).className).toBe(
    "secondary danger-button",
  );
  expect(utilityCss("danger-button")).toContain("color: var(--danger-text)");
});
