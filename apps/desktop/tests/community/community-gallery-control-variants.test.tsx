// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CommunitySearchForm, CommunityGalleryLoadMore } from "@msime/ui";
import {
  CommunityHarmonySearch,
  CommunityHarmonyLoadMore,
} from "../../../../packages/ui/src/community/community-skins";
import * as style from "../../../../packages/ui/src/community/community-style";

afterEach(cleanup);

for (const look of [undefined, "harmony"] as const) {
  test(`搜索外观 ${look ?? "desktop"} 保持输入、长度限制、受控值与零参数提交`, () => {
    const onChange = vi.fn();
    const onSubmit = vi.fn();
    const view = render(
      <CommunitySearchForm
        label="合成搜索"
        value="初始值"
        onChange={onChange}
        onSubmit={onSubmit}
        look={look}
      />,
    );
    const input = screen.getByRole("textbox", { name: "合成搜索" }) as HTMLInputElement;
    expect(input.placeholder).toBe("合成搜索");
    expect(input.value).toBe("初始值");
    expect(input.className).toBe(look ? style.harmonySearch : style.searchInput);
    expect(input.getAttribute("enterkeyhint")).toBe(look ? "search" : null);
    expect(screen.getByRole("search").className).toBe(look ? "m-0 min-w-0" : style.searchRow);
    expect(screen.queryByRole("button", { name: "搜索" }) !== null).toBe(!look);
    fireEvent.change(input, { target: { value: "👩‍💻".repeat(130) } });
    expect(onChange).toHaveBeenCalledWith("👩‍💻".repeat(128));
    view.rerender(
      <CommunitySearchForm
        label="合成搜索"
        value="新查询"
        onChange={onChange}
        onSubmit={onSubmit}
        look={look}
      />,
    );
    expect(input.value).toBe("新查询");
    const event = new Event("submit", { bubbles: true, cancelable: true });
    fireEvent(screen.getByRole("search"), event);
    expect(event.defaultPrevented).toBe(true);
    expect(onSubmit.mock.calls).toEqual([[]]);
    if (!look) {
      fireEvent.click(screen.getByRole("button", { name: "搜索" }));
      expect(onSubmit.mock.calls).toEqual([[], []]);
    }
  });

  test.each([
    [false, false],
    [true, false],
    [false, true],
    [true, true],
  ])(`分页外观 ${look ?? "desktop"}：hasMore=%s busy=%s`, (hasMore, busy) => {
    const onLoadMore = vi.fn();
    const { container } = render(
      <CommunityGalleryLoadMore
        hasMore={hasMore}
        busy={busy}
        loadingText="正在读取合成资源…"
        onLoadMore={onLoadMore}
        look={look}
      />,
    );
    const button = screen.queryByRole("button", { name: "加载更多" }) as HTMLButtonElement | null;
    expect(button !== null).toBe(hasMore);
    const status = screen.queryByRole("status");
    expect(status !== null).toBe(busy);
    if (button) {
      expect(button.disabled).toBe(busy);
      expect(button.className).toBe(look ? style.harmonyMore : `secondary ${style.more}`);
      fireEvent.click(button);
      expect(onLoadMore.mock.calls).toEqual(busy ? [] : [[]]);
    }
    if (status) {
      expect(status.textContent).toBe("正在读取合成资源…");
      expect(status.className).toBe(look ? style.harmonyNotice : style.notice);
    }
    if (!hasMore && !busy) expect(container.innerHTML).toBe("");
  });
}

test("皮肤页旧搜索与分页出口继续提供鸿蒙样式与交互", () => {
  const onSubmit = vi.fn();
  const onLoadMore = vi.fn();
  render(
    <>
      <CommunityHarmonySearch label="兼容搜索" value="" onChange={vi.fn()} onSubmit={onSubmit} />
      <CommunityHarmonyLoadMore
        hasMore
        busy={false}
        loadingText="合成读取"
        onLoadMore={onLoadMore}
      />
    </>,
  );
  const input = screen.getByRole("textbox", { name: "兼容搜索" });
  expect(input.className).toBe(style.harmonySearch);
  expect(input.getAttribute("enterkeyhint")).toBe("search");
  expect(screen.queryByRole("button", { name: "搜索" })).toBeNull();
  fireEvent.submit(screen.getByRole("search"));
  fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
  expect(onSubmit.mock.calls).toEqual([[]]);
  expect(onLoadMore.mock.calls).toEqual([[]]);
  expect(screen.getByRole("button", { name: "加载更多" }).className).toBe(style.harmonyMore);
});
