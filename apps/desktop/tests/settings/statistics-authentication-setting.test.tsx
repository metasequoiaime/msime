// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StatisticsAuthenticationRow } from "../../../../packages/ui/src/account/statistics-authentication-row";

afterEach(cleanup);

test("我的页认证选择保存在宿主，重新进入仍读取关闭状态", async () => {
  let stored = true;
  const client = {
    load: vi.fn(async () => stored),
    save: vi.fn(async (value: boolean) => {
      stored = value;
    }),
  };
  const first = render(<StatisticsAuthenticationRow client={client} />);
  const toggle = screen.getByRole("switch", { name: "统计页身份验证" });
  await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(false));
  expect((toggle as HTMLInputElement).checked).toBe(true);
  fireEvent.click(toggle);
  await waitFor(() => expect((toggle as HTMLInputElement).checked).toBe(false));
  expect(client.save).toHaveBeenCalledWith(false);
  first.unmount();
  render(<StatisticsAuthenticationRow client={client} />);
  await waitFor(() =>
    expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(false),
  );
  expect((screen.getByRole("switch") as HTMLInputElement).checked).toBe(false);
  fireEvent.click(screen.getByRole("switch"));
  await waitFor(() => expect((screen.getByRole("switch") as HTMLInputElement).checked).toBe(true));
  expect(stored).toBe(true);
});

test("保存失败不显示为已关闭，读取失败不允许覆盖宿主选择", async () => {
  const client = {
    load: vi.fn(async () => true),
    save: vi.fn(async () => {
      throw new Error("synthetic save failure");
    }),
  };
  const first = render(<StatisticsAuthenticationRow client={client} />);
  const toggle = screen.getByRole("switch");
  await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(false));
  fireEvent.click(toggle);
  await waitFor(() => expect(client.save).toHaveBeenCalledOnce());
  await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(false));
  expect((toggle as HTMLInputElement).checked).toBe(true);
  first.unmount();
  client.load.mockRejectedValueOnce(new Error("synthetic load failure"));
  render(<StatisticsAuthenticationRow client={client} />);
  expect(await screen.findByText("认证设置读取失败，请重新打开此页。")).toBeTruthy();
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
});

test("替换认证客户端后，旧保存回执不能覆盖新客户端的选择", async () => {
  let resolveSave!: () => void;
  const oldSave = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        resolveSave = resolve;
      }),
  );
  const oldClient = {
    load: vi.fn().mockResolvedValue(false),
    save: oldSave,
  };
  const nextClient = {
    load: vi.fn().mockResolvedValue(false),
    save: vi.fn().mockResolvedValue(undefined),
  };
  const view = render(<StatisticsAuthenticationRow client={oldClient} />);
  const toggle = screen.getByRole("switch", { name: "统计页身份验证" });
  await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(false));
  fireEvent.click(toggle);
  await waitFor(() => expect(oldSave).toHaveBeenCalledWith(true));
  view.rerender(<StatisticsAuthenticationRow client={nextClient} />);
  await waitFor(() => expect((screen.getByRole("switch") as HTMLInputElement).checked).toBe(false));
  await act(async () => {
    resolveSave();
    await Promise.resolve();
  });
  expect((screen.getByRole("switch") as HTMLInputElement).checked).toBe(false);
  expect(nextClient.save).not.toHaveBeenCalled();
});
