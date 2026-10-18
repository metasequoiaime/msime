// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useSettingsTabs } from "../../../../packages/ui/src/settings/use-settings-tabs";

afterEach(cleanup);

type Section = "first" | "second" | "third";

const titles: Record<Section, string> = { first: "第一项", second: "第二项", third: "第三项" };

function Sections({ shown }: { shown: Section[] }) {
  const { tabList, panel } = useSettingsTabs(
    shown.map((id) => ({ id, title: titles[id] })),
    "分区",
    true,
  );
  return (
    <>
      {tabList}
      {panel("first", <p>第一项内容</p>)}
      {panel("second", <p>第二项内容</p>)}
      {panel("third", <p>第三项内容</p>)}
    </>
  );
}

function selectedTab(): string | null {
  return within(screen.getByRole("tablist", { name: "分区" })).getByRole("tab", { selected: true })
    .textContent;
}

// 默认标签（第一个）晚于首次绘制才出现时，用户还没点过标签，就落到它上面。
test("an untouched tab set follows its first tab when that tab arrives late", () => {
  const { rerender } = render(<Sections shown={["second", "third"]} />);
  expect(selectedTab()).toBe("第二项");
  rerender(<Sections shown={["first", "second", "third"]} />);
  expect(selectedTab()).toBe("第一项");
  // `getByText` 不排除隐藏元素，这里用 role 查询确认用户看到的是哪一块。
  expect(screen.getByRole("tabpanel").textContent).toBe("第一项内容");
});

test("a tab the user chose is kept when the default tab arrives", () => {
  const { rerender } = render(<Sections shown={["second", "third"]} />);
  fireEvent.click(screen.getByRole("tab", { name: "第三项" }));
  rerender(<Sections shown={["first", "second", "third"]} />);
  expect(selectedTab()).toBe("第三项");
  expect(screen.getByRole("tabpanel").textContent).toBe("第三项内容");
});
