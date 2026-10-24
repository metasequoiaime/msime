// @vitest-environment jsdom
import { presentAccountName } from "../support/account-name";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { AccountPage } from "../../../../packages/ui/src/account/account-page";
import type {
  AccountClient,
  AccountPreferences,
  AccountPreferenceSchema,
  SettingsSyncClient,
} from "@msime/ui";

afterEach(cleanup);

const user = { id: "synthetic-user", displayName: "合成用户", createdAt: "2026-01-01T00:00:00Z" };
const schema: AccountPreferenceSchema = {
  fields: { "input.schema": { type: "string" } },
  maximumBytes: 65536,
  updateMode: "replace",
  revisionRequired: true,
};
const cloud: AccountPreferences = { revision: 7, settings: { "input.schema": "quanpin" } };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
}

function syncClient(overrides: Partial<SettingsSyncClient> = {}): SettingsSyncClient {
  return {
    schema: vi.fn(async () => schema),
    load: vi.fn(async () => cloud),
    upload: vi.fn(async () => ({ ...cloud, revision: 8 })),
    apply: vi.fn(async () => {}),
    ...overrides,
  };
}

function account(settingsSync: SettingsSyncClient): AccountClient {
  return {
    status: async () => ({ user }),
    providers: async () => ({ email: true, phone: false }),
    profile: async () => ({ user, providers: ["email"] }),
    requestCode: vi.fn(),
    login: vi.fn(),
    namePresentation: presentAccountName,
    rename: vi.fn(),
    logout: vi.fn(),
    deleteAccount: vi.fn(),
    clearExpired: vi.fn(),
    settingsSync,
  };
}

test("同步首次加载等待字段表与快照，空快照只禁用应用并可取消上传确认", async () => {
  const pendingSchema = deferred<AccountPreferenceSchema>();
  const sync = syncClient({
    schema: () => pendingSchema.promise,
    load: async () => ({ revision: 0, settings: {} }),
  });
  render(<AccountPage client={account(sync)} />);
  const refresh = await screen.findByRole("button", { name: "刷新云端设置" });
  await waitFor(() => expect((refresh as HTMLButtonElement).disabled).toBe(true));
  expect((screen.getByRole("button", { name: "上传本机设置" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(screen.queryByText("云端版本：0")).toBeNull();
  await act(async () => pendingSchema.resolve(schema));
  expect(await screen.findByText("云端版本：0")).not.toBeNull();
  expect((refresh as HTMLButtonElement).disabled).toBe(false);
  expect(
    (screen.getByRole("button", { name: "下载并应用云端设置" }) as HTMLButtonElement).disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "上传本机设置" }));
  expect(screen.getByRole("alertdialog", { name: "确认上传本机设置" })).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(sync.upload).not.toHaveBeenCalled();
});

test("刷新失败保留现有快照，后续刷新成功清除错误并应用最新快照", async () => {
  const next = { revision: 9, settings: { "input.schema": "shuangpin" } };
  const load = vi
    .fn()
    .mockResolvedValueOnce(cloud)
    .mockRejectedValueOnce({ code: "account_conflict" })
    .mockResolvedValueOnce(next);
  const sync = syncClient({ load });
  render(<AccountPage client={account(sync)} />);
  expect(await screen.findByText("云端版本：7")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "刷新云端设置" }));
  expect(await screen.findByText("云端设置已被其他设备更新，请刷新后重新确认。")).not.toBeNull();
  expect(screen.getByText("云端版本：7")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "刷新云端设置" }));
  expect(await screen.findByText("云端版本：9")).not.toBeNull();
  expect(screen.queryByText("云端设置已被其他设备更新，请刷新后重新确认。")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下载并应用云端设置" }));
  expect(sync.apply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认应用" }));
  await waitFor(() => expect(sync.apply).toHaveBeenCalledWith(user.id, next));
  expect(await screen.findByText("已应用云端设置。请重新打开键盘使部分设置生效。")).not.toBeNull();
});

test.each([false, true])(
  "替换同步客户端后立即读取新快照并隔离旧读取（StrictMode=%s）",
  async (strict) => {
    const pending = deferred<AccountPreferences>();
    const oldSync = syncClient({ load: vi.fn(() => pending.promise) });
    const client = account(oldSync);
    const page = () =>
      strict ? (
        <StrictMode>
          <AccountPage client={client} />
        </StrictMode>
      ) : (
        <AccountPage client={client} />
      );
    const view = render(page());
    await waitFor(() => expect(oldSync.load).toHaveBeenCalled());
    const next = { revision: 10, settings: { "input.schema": "shuangpin" } };
    const newSync = syncClient({ load: async () => next });
    client.settingsSync = newSync;
    view.rerender(page());
    expect(await screen.findByText("云端版本：10")).not.toBeNull();
    await act(async () => pending.resolve(cloud));
    expect(screen.queryByText("云端版本：7")).toBeNull();
    expect(
      (screen.getByRole("button", { name: "上传本机设置" }) as HTMLButtonElement).disabled,
    ).toBe(false);
  },
);

test.each(["success", "failure"])("旧上传迟到的 %s 不结束新客户端上传", async (outcome) => {
  const oldUpload = deferred<AccountPreferences>();
  const nextUpload = deferred<AccountPreferences>();
  const oldSync = syncClient({ upload: () => oldUpload.promise });
  const client = account(oldSync);
  const view = render(<AccountPage client={client} />);
  expect(await screen.findByText("云端版本：7")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "上传本机设置" }));
  fireEvent.click(screen.getByRole("button", { name: "确认上传" }));
  const next = { revision: 10, settings: { "input.schema": "shuangpin" } };
  client.settingsSync = syncClient({ load: async () => next, upload: () => nextUpload.promise });
  view.rerender(<AccountPage client={client} />);
  expect(await screen.findByText("云端版本：10")).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "上传本机设置" }));
  fireEvent.click(screen.getByRole("button", { name: "确认上传" }));
  await act(async () => {
    if (outcome === "success") oldUpload.resolve({ ...cloud, revision: 8 });
    else oldUpload.reject({ code: "account_conflict" });
  });
  expect((screen.getByRole("button", { name: "刷新云端设置" }) as HTMLButtonElement).disabled).toBe(
    true,
  );
  expect(screen.getByText("云端版本：10")).not.toBeNull();
  expect(screen.queryByText("本机设置已上传。")).toBeNull();
  expect(screen.queryByText("云端设置已被其他设备更新，请刷新后重新确认。")).toBeNull();
  await act(async () => nextUpload.resolve({ ...next, revision: 11 }));
  expect(screen.getByText("云端版本：11")).not.toBeNull();
  expect(screen.getByText("本机设置已上传。")).not.toBeNull();
});
