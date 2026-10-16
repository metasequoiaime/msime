// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, render, renderHook, screen, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useWubiCodeLookup, WubiCodeLookupResults, type WubiCodeLookup } from "@msime/ui";

afterEach(cleanup);

const answer: WubiCodeLookup = {
  phrase: { text: "合成", wubi86: ["synthetic-phrase"], wubi98: [] },
  characters: [
    { text: "合", wubi86: ["synthetic-short", "synthetic-full"], wubi98: ["synthetic-98"] },
  ],
  truncated: false,
};
function deferred() {
  let resolve!: (value: WubiCodeLookup) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<WubiCodeLookup>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("公共查码状态拒绝空白，保留请求原文并同步防重", async () => {
  const request = deferred();
  const lookup = vi.fn(() => request.promise);
  const hook = renderHook(() => useWubiCodeLookup({ lookup }), { wrapper: StrictMode });
  expect(hook.result.current.canSearch).toBe(false);
  act(() => hook.result.current.edit("  "));
  await act(async () => hook.result.current.search());
  expect(lookup).not.toHaveBeenCalled();
  act(() => hook.result.current.edit(" 合成 "));
  expect(hook.result.current.canSearch).toBe(true);
  act(() => {
    void hook.result.current.search();
    void hook.result.current.search();
  });
  expect(lookup).toHaveBeenCalledExactlyOnceWith(" 合成 ");
  expect(hook.result.current.busy).toBe(true);
  expect(hook.result.current.canSearch).toBe(false);
  await act(async () => request.resolve(answer));
  expect(hook.result.current.result).toEqual(answer);
  expect(hook.result.current.canSearch).toBe(true);
});

test.each(["成功", "失败"])("编辑文字后隔离旧%s和收尾，新请求可以立即开始", async (outcome) => {
  const old = deferred();
  const next = deferred();
  const lookup = vi.fn().mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise);
  const hook = renderHook(() => useWubiCodeLookup({ lookup }));
  act(() => hook.result.current.edit("合成旧查询"));
  act(() => {
    void hook.result.current.search();
  });
  act(() => hook.result.current.edit("合成新查询"));
  expect(hook.result.current.canSearch).toBe(true);
  act(() => {
    void hook.result.current.search();
  });
  await act(async () => {
    if (outcome === "成功") old.resolve(answer);
    else old.reject(new Error("合成旧失败"));
  });
  expect(hook.result.current.result).toBeNull();
  expect(hook.result.current.error).toBe("");
  expect(hook.result.current.busy).toBe(true);
  const current = { ...answer, phrase: null };
  await act(async () => next.resolve(current));
  expect(hook.result.current.result).toEqual(current);
});

test("当前查询失败清除旧结果，编辑后清除错误", async () => {
  const lookup = vi.fn().mockResolvedValueOnce(answer).mockRejectedValueOnce(new Error("合成失败"));
  const hook = renderHook(() => useWubiCodeLookup({ lookup }));
  act(() => hook.result.current.edit("合成查询"));
  await act(async () => hook.result.current.search());
  expect(hook.result.current.result).toEqual(answer);
  await act(async () => hook.result.current.search());
  expect(hook.result.current.result).toBeNull();
  expect(hook.result.current.error).toBe("查码失败，请稍后重试。");
  act(() => hook.result.current.edit("合成新查询"));
  expect(hook.result.current.error).toBe("");
});

test("更换客户端清掉已完成结果，保留输入且不自动重查", async () => {
  const lookup = vi.fn().mockResolvedValue(answer);
  const next = vi.fn().mockResolvedValue({ ...answer, phrase: null });
  const hook = renderHook(({ lookup }) => useWubiCodeLookup({ lookup }), {
    initialProps: { lookup },
  });
  act(() => hook.result.current.edit("合成查询"));
  await act(async () => hook.result.current.search());
  hook.rerender({ lookup: next });
  expect(hook.result.current.result).toBeNull();
  expect(hook.result.current.error).toBe("");
  expect(hook.result.current.text).toBe("合成查询");
  expect(hook.result.current.canSearch).toBe(true);
  expect(next).not.toHaveBeenCalled();
});

test("公共结果组件独立展示词在前、当前码表、缺码与截断", () => {
  const view = render(
    <WubiCodeLookupResults result={{ ...answer, truncated: true }} profile="wubi98" />,
  );
  const table = screen.getByRole("table", { name: "五笔编码" });
  expect(
    within(table)
      .getAllByRole("row")
      .map((row) => row.textContent),
  ).toEqual([
    "文字86 五笔98 五笔（当前）",
    "合成synthetic-phrase—",
    "合synthetic-short synthetic-fullsynthetic-98",
  ]);
  expect(screen.getByText("字太多，只列出了前 1 个不同的字。")).toBeTruthy();
  view.rerender(
    <WubiCodeLookupResults result={{ phrase: null, characters: [], truncated: false }} />,
  );
  expect(screen.queryByRole("table")).toBeNull();
  expect(screen.getByText("没有可以查的汉字。")).toBeTruthy();
  view.rerender(<WubiCodeLookupResults result={null} />);
  expect(screen.queryByRole("status")).toBeNull();
});
