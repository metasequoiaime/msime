// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useCommunitySkinCategoryFilter } from "../../../../packages/ui/src/community/use-community-skin-category-filter";

afterEach(cleanup);

test("分类失败退回上次成功显示的分类，每次调用独立持有状态", async () => {
  const first = renderHook(useCommunitySkinCategoryFilter);
  const second = renderHook(useCommunitySkinCategoryFilter);
  await act(async () => first.result.current.change("nature", async () => true));
  expect(first.result.current.category).toBe("nature");
  expect(second.result.current.category).toBeNull();
  await act(async () => first.result.current.change("food", async () => false));
  expect(first.result.current.category).toBe("nature");
  expect(first.result.current.request.current).toBe("nature");
});

test("过期的分类完成不覆盖当前分类，返回全部仍发起新读取", async () => {
  const { result } = renderHook(useCommunitySkinCategoryFilter);
  let finish!: (value: boolean) => void;
  const pending = new Promise<boolean>((resolve) => {
    finish = resolve;
  });
  let changing!: Promise<void>;
  act(() => {
    changing = result.current.change("nature", () => pending);
  });
  expect(result.current.category).toBe("nature");
  const reload = vi.fn(async () => true);
  await act(async () => result.current.change(null, reload));
  expect(reload).toHaveBeenCalledOnce();
  await act(async () => {
    finish(true);
    await changing;
  });
  expect(result.current.category).toBeNull();
  expect(result.current.request.current).toBeNull();
  await act(async () => result.current.change(null, reload));
  expect(reload).toHaveBeenCalledOnce();
});
