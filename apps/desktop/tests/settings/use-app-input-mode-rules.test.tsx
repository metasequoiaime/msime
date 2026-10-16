// @vitest-environment jsdom
import { StrictMode, useState } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useAppInputModeRules, type AppInputModeRule, type PickedApplication } from "@msime/ui";

afterEach(cleanup);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("公共规则状态独立完成手填校验、排序、改模式和移除", () => {
  const hook = renderHook(() => {
    const [value, onChange] = useState<Record<string, AppInputModeRule>>({ "z.exe": "english" });
    return useAppInputModeRules({ value, onChange, windows: true });
  });
  act(() => hook.result.current.edit("missing-extension"));
  act(() => hook.result.current.addTyped());
  expect(hook.result.current.warning).toContain(".exe");
  act(() => hook.result.current.edit(" C:\\Synthetic\\A.EXE "));
  expect(hook.result.current.warning).toBeNull();
  act(() => hook.result.current.addTyped());
  expect(hook.result.current.draft).toBe("");
  expect(hook.result.current.entries).toEqual([
    ["a.exe", "chinese"],
    ["z.exe", "english"],
  ]);
  act(() => hook.result.current.changeMode("a.exe", "english"));
  expect(hook.result.current.entries[0]).toEqual(["a.exe", "english"]);
  act(() => hook.result.current.remove("z.exe"));
  expect(hook.result.current.entries).toEqual([["a.exe", "english"]]);
});

test("名称缺失时缓存标识回退，模式修改不重查", async () => {
  const names = vi.fn(async (_ids: string[]) => ({}));
  const hook = renderHook(
    ({ mode }) =>
      useAppInputModeRules({
        value: { "org.example.synthetic": mode },
        windows: false,
        applicationNames: names,
        onChange: vi.fn(),
      }),
    { initialProps: { mode: "chinese" as AppInputModeRule } },
  );
  await waitFor(() =>
    expect(hook.result.current.names["org.example.synthetic"]).toBe("org.example.synthetic"),
  );
  hook.rerender({ mode: "english" });
  expect(names).toHaveBeenCalledOnce();
});

test("选择器回包使用最新规则和回调，回调变化不重置进行中的操作", async () => {
  const request = deferred<PickedApplication | null>();
  const pickApplication = vi.fn(() => request.promise);
  const oldChange = vi.fn();
  const nextChange = vi.fn();
  const hook = renderHook(
    ({ value, onChange }) =>
      useAppInputModeRules({ value, onChange, windows: false, pickApplication }),
    {
      initialProps: {
        value: { "org.example.deleted": "chinese" } as Record<string, AppInputModeRule>,
        onChange: oldChange,
      },
    },
  );
  act(() => {
    void hook.result.current.pick?.();
  });
  hook.rerender({ value: { "org.example.current": "english" }, onChange: nextChange });
  expect(hook.result.current.picking).toBe(true);
  await act(async () => request.resolve({ id: "org.example.new", name: "合成新应用" }));
  expect(oldChange).not.toHaveBeenCalled();
  expect(nextChange).toHaveBeenCalledExactlyOnceWith({
    "org.example.current": "english",
    "org.example.new": "chinese",
  });
  expect(hook.result.current.names["org.example.new"]).toBe("合成新应用");
});

test("StrictMode 中选择操作同步防重，取消后可重新选择，错误沿用领域说明", async () => {
  const request = deferred<PickedApplication | null>();
  const pickApplication = vi
    .fn()
    .mockReturnValueOnce(request.promise)
    .mockRejectedValueOnce({ code: "not_application" });
  const onChange = vi.fn();
  const hook = renderHook(
    () => useAppInputModeRules({ windows: false, pickApplication, onChange }),
    { wrapper: StrictMode },
  );
  act(() => {
    void hook.result.current.pick?.();
    void hook.result.current.pick?.();
  });
  expect(pickApplication).toHaveBeenCalledOnce();
  await act(async () => request.resolve(null));
  expect(hook.result.current.picking).toBe(false);
  expect(onChange).not.toHaveBeenCalled();
  await act(async () => hook.result.current.pick?.());
  expect(hook.result.current.warning).toBe("所选项目不是应用程序，无法加入应用例外。");
});

test("选择期间规则填满后按当前上限拒绝，不丢已有规则", async () => {
  const request = deferred<PickedApplication | null>();
  const pickApplication = vi.fn(() => request.promise);
  const onChange = vi.fn();
  const hook = renderHook(
    ({ value }) => useAppInputModeRules({ windows: true, value, pickApplication, onChange }),
    { initialProps: { value: {} as Record<string, AppInputModeRule> } },
  );
  act(() => {
    void hook.result.current.pick?.();
  });
  const full = Object.fromEntries(
    Array.from({ length: 32 }, (_, index) => [`synthetic-${index}.exe`, "english" as const]),
  );
  hook.rerender({ value: full });
  await act(async () => request.resolve({ id: "new.exe", name: "合成新应用" }));
  expect(hook.result.current.full).toBe(true);
  expect(hook.result.current.warning).toContain("最多 32 个");
  expect(onChange).not.toHaveBeenCalled();
});

test.each(["成功", "失败"])("平台更换后旧选择%s不能写回或报警", async (outcome) => {
  const request = deferred<PickedApplication | null>();
  const pickApplication = vi.fn(() => request.promise);
  const onChange = vi.fn();
  const hook = renderHook(
    ({ windows }) => useAppInputModeRules({ windows, pickApplication, onChange }),
    { initialProps: { windows: false } },
  );
  act(() => {
    void hook.result.current.pick?.();
  });
  hook.rerender({ windows: true });
  await act(async () => {
    if (outcome === "成功") request.resolve({ id: "org.example.old", name: "合成旧应用" });
    else request.reject(new Error("合成旧平台错误"));
  });
  expect(onChange).not.toHaveBeenCalled();
  expect(hook.result.current.warning).toBeNull();
  expect(hook.result.current.picking).toBe(false);
});
