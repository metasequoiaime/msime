// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountPage } from "../../../../packages/ui/src/account/account-page";
import type { AppIconClient, AppIconInfo } from "@msime/ui";

afterEach(cleanup);

const classic: AppIconInfo = { supported: true, selected: "classic" };
const forest = "杉林，杉叶青绿，沉静自然";
const sky = "晴空，清透蓝调，轻盈明亮";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

function page(client: AppIconClient, strict: boolean) {
  const content = <AccountPage appIcon={client} />;
  return strict ? <StrictMode>{content}</StrictMode> : content;
}

test("图标首次读取失败后更换客户端会清除错误并显示新设备能力", async () => {
  const initial = deferred<AppIconInfo>();
  const view = render(page({ info: () => initial.promise, set: vi.fn() }, false));
  expect(screen.getByText("正在读取图标状态…")).not.toBeNull();
  expect(screen.queryByRole("button", { name: forest })).toBeNull();
  await act(async () => initial.reject(new Error("synthetic")));
  expect(screen.getByRole("alert").textContent).toBe("暂时无法读取 App 图标状态，请稍后重试。");

  const set = vi.fn();
  view.rerender(
    page({ info: async () => ({ supported: false, selected: "classic" }), set }, false),
  );
  expect(await screen.findByText("当前设备暂不支持更换 App 图标。")).not.toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
  const button = screen.getByRole("button", { name: forest }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  fireEvent.click(button);
  expect(set).not.toHaveBeenCalled();
});

test("图标切换返回其他图标时显示失败，下一次成功清除提示", async () => {
  const set = vi.fn().mockResolvedValueOnce(classic).mockResolvedValueOnce({
    supported: true,
    selected: "sky",
  });
  render(page({ info: async () => classic, set }, false));
  const button = (await screen.findByRole("button", { name: forest })) as HTMLButtonElement;
  fireEvent.click(button);
  expect((await screen.findByRole("alert")).textContent).toBe("图标未能更换，请稍后重试。");
  expect(button.disabled).toBe(false);
  expect(button.getAttribute("aria-pressed")).toBe("false");

  const next = screen.getByRole("button", { name: sky });
  fireEvent.click(next);
  await waitFor(() => expect(next.getAttribute("aria-pressed")).toBe("true"));
  expect(screen.queryByRole("alert")).toBeNull();
  expect(set.mock.calls).toEqual([["forest"], ["sky"]]);
});

test("切换及重读均失败时保留已知图标并允许重试", async () => {
  const info = vi.fn().mockResolvedValueOnce(classic).mockRejectedValue(new Error("synthetic"));
  const set = vi.fn().mockRejectedValueOnce(new Error("synthetic")).mockResolvedValueOnce({
    supported: true,
    selected: "forest",
  });
  render(page({ info, set }, false));
  const button = (await screen.findByRole("button", { name: forest })) as HTMLButtonElement;
  fireEvent.click(button);
  expect((await screen.findByRole("alert")).textContent).toBe("图标未能更换，请稍后重试。");
  expect(button.disabled).toBe(false);
  expect(
    screen.getByRole("button", { name: "原版，经典黑白，简洁如初" }).getAttribute("aria-pressed"),
  ).toBe("true");
  fireEvent.click(button);
  await waitFor(() => expect(button.getAttribute("aria-pressed")).toBe("true"));
  expect(screen.queryByRole("alert")).toBeNull();
  expect(info).toHaveBeenCalledTimes(2);
});

test.each(["success", "failure"])("旧客户端重读的迟到 %s 不覆盖新图标", async (outcome) => {
  const reread = deferred<AppIconInfo>();
  const info = vi
    .fn()
    .mockResolvedValueOnce(classic)
    .mockImplementationOnce(() => reread.promise);
  const view = render(
    page(
      {
        info,
        set: async () => {
          throw new Error("synthetic");
        },
      },
      false,
    ),
  );
  fireEvent.click(await screen.findByRole("button", { name: forest }));
  await waitFor(() => expect(info).toHaveBeenCalledTimes(2));
  const nextClient = { info: async () => ({ supported: true, selected: "sky" }), set: vi.fn() };
  view.rerender(page(nextClient, false));
  const button = (await screen.findByRole("button", { name: sky })) as HTMLButtonElement;
  await waitFor(() => expect(button.getAttribute("aria-pressed")).toBe("true"));
  await act(async () => {
    if (outcome === "success") reread.resolve({ supported: true, selected: "forest" });
    else reread.reject(new Error("synthetic"));
  });
  expect(button.getAttribute("aria-pressed")).toBe("true");
  expect(button.disabled).toBe(false);
  expect(screen.queryByRole("alert")).toBeNull();
});

test.each([false, true])(
  "图标客户端更换后不等待旧切换结果就能继续选择（StrictMode=%s）",
  async (strict) => {
    const oldChange = deferred<AppIconInfo>();
    const oldClient = {
      info: vi.fn().mockResolvedValue(classic),
      set: vi.fn(() => oldChange.promise),
    };
    const nextClient = {
      info: vi.fn().mockResolvedValue(classic),
      set: vi.fn().mockResolvedValue({ supported: true, selected: "forest" }),
    };
    const view = render(page(oldClient, strict));
    fireEvent.click(await screen.findByRole("button", { name: forest }));
    await waitFor(() => expect(oldClient.set).toHaveBeenCalledWith("forest"));
    expect((screen.getByRole("button", { name: forest }) as HTMLButtonElement).disabled).toBe(true);

    view.rerender(page(nextClient, strict));
    const nextForest = (await screen.findByRole("button", { name: forest })) as HTMLButtonElement;
    await waitFor(() => expect(nextForest.disabled).toBe(false));
    expect(nextForest.textContent).not.toContain("更换中");
    fireEvent.click(nextForest);
    await waitFor(() => expect(nextForest.getAttribute("aria-pressed")).toBe("true"));
    expect(nextClient.set).toHaveBeenCalledWith("forest");
    await act(async () => oldChange.resolve({ supported: true, selected: "dusk" }));
    expect(nextForest.getAttribute("aria-pressed")).toBe("true");
  },
);

test.each(["success", "failure"])("旧图标切换的迟到 %s 不结束新客户端的切换", async (outcome) => {
  const oldChange = deferred<AppIconInfo>();
  const nextChange = deferred<AppIconInfo>();
  const oldClient = {
    info: vi.fn().mockResolvedValue(classic),
    set: vi.fn(() => oldChange.promise),
  };
  const nextClient = {
    info: vi.fn().mockResolvedValue(classic),
    set: vi.fn(() => nextChange.promise),
  };
  const view = render(page(oldClient, true));
  fireEvent.click(await screen.findByRole("button", { name: forest }));
  await waitFor(() => expect(oldClient.set).toHaveBeenCalledOnce());
  view.rerender(page(nextClient, true));
  const nextSky = (await screen.findByRole("button", { name: sky })) as HTMLButtonElement;
  await waitFor(() => expect(nextSky.disabled).toBe(false));
  fireEvent.click(nextSky);
  await waitFor(() => expect(nextClient.set).toHaveBeenCalledWith("sky"));

  await act(async () => {
    if (outcome === "success") oldChange.resolve({ supported: true, selected: "forest" });
    else oldChange.reject({ code: "app_icon" });
  });
  expect(nextSky.disabled).toBe(true);
  expect(nextSky.textContent).toContain("更换中");
  expect(screen.queryByRole("alert")).toBeNull();
  await act(async () => nextChange.resolve({ supported: true, selected: "sky" }));
  expect(nextSky.disabled).toBe(false);
  expect(nextSky.getAttribute("aria-pressed")).toBe("true");
});
