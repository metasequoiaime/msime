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
