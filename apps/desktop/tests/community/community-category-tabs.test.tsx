// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { CommunityCategoryTabs } from "@msime/ui";
import * as style from "../../../../packages/ui/src/community/community-style";

afterEach(cleanup);

test.each([
  { hasSkins: true, look: undefined, labels: ["皮肤", "词库", "回复模板"] },
  { hasSkins: false, look: undefined, labels: ["词库", "回复模板"] },
  { hasSkins: true, look: "harmony" as const, labels: ["皮肤", "词库", "短语"] },
  { hasSkins: false, look: "harmony" as const, labels: ["词库", "短语"] },
])(
  "category labels follow host capabilities and appearance: $hasSkins / $look",
  ({ hasSkins, look, labels }) => {
    render(
      <CommunityCategoryTabs
        category="reply"
        hasSkins={hasSkins}
        look={look}
        onCategoryChange={vi.fn()}
      />,
    );
    const list = screen.getByRole("tablist", { name: "社区分类" });
    expect(list.className).toBe(look === "harmony" ? style.harmonySegments : style.categoryTabs);
    const tabs = within(list).getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual(labels);
    expect(tabs.map((tab) => tab.getAttribute("aria-selected"))).toEqual(
      labels.map((_, index) => String(index === labels.length - 1)),
    );
  },
);

test("category tabs emit the category while selection stays controlled by their caller", () => {
  const onCategoryChange = vi.fn();
  const view = render(
    <CommunityCategoryTabs category="skin" hasSkins onCategoryChange={onCategoryChange} />,
  );
  fireEvent.click(screen.getByRole("tab", { name: "词库" }));
  expect(onCategoryChange).toHaveBeenCalledExactlyOnceWith("dictionary");
  expect(screen.getByRole("tab", { name: "皮肤" }).getAttribute("aria-selected")).toBe("true");
  view.rerender(
    <CommunityCategoryTabs category="dictionary" hasSkins onCategoryChange={onCategoryChange} />,
  );
  expect(screen.getByRole("tab", { name: "词库" }).getAttribute("aria-selected")).toBe("true");
  fireEvent.click(screen.getByRole("tab", { name: "回复模板" }));
  expect(onCategoryChange).toHaveBeenLastCalledWith("reply");
  fireEvent.click(screen.getByRole("tab", { name: "皮肤" }));
  expect(onCategoryChange).toHaveBeenLastCalledWith("skin");
});

test("category buttons do not submit a surrounding form", () => {
  const onSubmit = vi.fn((event: React.SubmitEvent<HTMLFormElement>) => event.preventDefault());
  const onCategoryChange = vi.fn();
  render(
    <form onSubmit={onSubmit}>
      <CommunityCategoryTabs category="skin" hasSkins onCategoryChange={onCategoryChange} />
    </form>,
  );
  screen.getByRole("tab", { name: "词库" }).click();
  expect(onCategoryChange).toHaveBeenCalledExactlyOnceWith("dictionary");
  expect(onSubmit).not.toHaveBeenCalled();
});
