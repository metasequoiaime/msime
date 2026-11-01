// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CandidateShortcutsSection } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const navigation = {
  minus_equal: true,
  comma_period: false,
  brackets: false,
  tab: true,
  page_up_down: false,
  mouse_wheel: true,
  arrows: true,
};

test("candidate shortcuts report number-row selection changes", () => {
  const onNumberRowSelectionChange = vi.fn();
  render(
    <CandidateShortcutsSection
      navigation={navigation}
      numberRowSelection={true}
      showNumberRowSelection
      mobile={false}
      onNumberRowSelectionChange={onNumberRowSelectionChange}
    />,
  );

  fireEvent.click(screen.getByLabelText("数字键选词"));
  expect(onNumberRowSelectionChange).toHaveBeenCalledWith(false);
  expect(screen.getByText("Space 或 1–9")).toBeTruthy();
});

test("candidate shortcuts only show enabled navigation rows", () => {
  render(
    <CandidateShortcutsSection
      navigation={{ ...navigation, arrows: false }}
      numberRowSelection={false}
      showNumberRowSelection={false}
      mobile
      onNumberRowSelectionChange={vi.fn()}
    />,
  );

  expect(screen.getByText("候选栏翻页")).toBeTruthy();
  expect(screen.getByText("鼠标滚轮")).toBeTruthy();
  expect(screen.queryByText("Space 或 1–9")).toBeNull();
});

const allPaging = {
  minus_equal: true,
  comma_period: true,
  brackets: true,
  tab: true,
  page_up_down: true,
  mouse_wheel: false,
  arrows: false,
};

// 行尾控件放在 Row 的最后一格；说明放在标题下方，和标题同在一列文字里。
// 行的文字列：标题外面是标题与「?」那一行，再外面才是放标题和常驻说明的文字列。
const pagingRow = () =>
  screen.getByText("向前 / 向后翻页").closest("[data-row-title]")!.parentElement!.parentElement!;

test("several paging key groups sit on their own line below the title", () => {
  render(
    <CandidateShortcutsSection
      navigation={allPaging}
      numberRowSelection={true}
      showNumberRowSelection={false}
      mobile={false}
      onNumberRowSelectionChange={vi.fn()}
    />,
  );

  const keys = Array.from(pagingRow().querySelectorAll("kbd")).map((kbd) => kbd.textContent);
  expect(keys).toEqual(["- / =", ", / .", "[ / ]", "Shift+Tab / Tab", "Page Up / Page Down"]);
});

test("a single paging key group stays at the trailing edge like the other rows", () => {
  render(
    <CandidateShortcutsSection
      navigation={{
        ...allPaging,
        comma_period: false,
        brackets: false,
        tab: false,
        page_up_down: false,
      }}
      numberRowSelection={true}
      showNumberRowSelection={false}
      mobile={false}
      onNumberRowSelectionChange={vi.fn()}
    />,
  );

  expect(pagingRow().querySelector("kbd")).toBeNull();
  expect(screen.getByText("- / =").tagName).toBe("KBD");
});

test("desktop folds the reference rows into 按键速查 and drops an empty 候选操作 group", () => {
  render(
    <CandidateShortcutsSection
      navigation={navigation}
      numberRowSelection
      showNumberRowSelection={false}
      mobile={false}
      onNumberRowSelectionChange={vi.fn()}
      extraReference={<span>额外的参考行</span>}
    />,
  );
  expect(screen.queryByText("候选操作")).toBeNull();
  const reference = screen.getByRole("region", { name: "按键速查" });
  // 收在「更多选项」折叠区里，展开前不占地方。
  const fold = reference.querySelector("details");
  expect(fold).not.toBeNull();
  expect(fold?.open).toBe(false);
  expect(fold?.textContent).toContain("Home / End");
  expect(fold?.textContent).toContain("额外的参考行");
});
