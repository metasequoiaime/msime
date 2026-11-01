// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import {
  DICTIONARY_SYNC_NOTE,
  SettingsPage,
  dictionarySyncBudgetWaitMessage,
  dictionarySyncDeviceGroups,
  dictionarySyncErrorMessage,
  dictionarySyncRun,
  dictionarySyncStatus,
  withDictionarySyncWebDav,
  type DictionaryClient,
  type DictionarySyncAction,
  type Preferences,
  type SettingsClient,
} from "@msime/ui";
import { dictionarySyncDelay } from "../../../../packages/ui/src/settings/dictionary-sync";
import {
  HOST_USER_WORDS_CHANGED_EVENT,
  createDictionaryLocalChanges,
  useDictionarySync,
  withDictionaryChangeNotices,
} from "../../../../packages/ui/src/settings/use-dictionary-sync";
import { settingsFormReady, saveSettingsNow } from "../support/settings-form";
import { answerConfirm } from "../support/confirm";
import { testHost } from "../support/host";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

// 合成的网盘、账号、应用密码和设备，不对应任何真实账号。
const WEBDAV = {
  url: "https://dav.example.test/dav/",
  username: "synthetic@example.test",
  password: "synthetic-app-password",
};
const LOCAL_ID = "0123456789abcdef0123456789abcdef";
const OTHER_ID = "fedcba9876543210fedcba9876543210";
const LOCAL_CODE = "1111 2222 3333 4444";
const OTHER_CODE = "5555 6666 7777 8888";

const base: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 5,
  learning: true,
  chinese_punctuation: true,
};

function snapshot(preferences: Preferences, revision = 7) {
  return { format_version: 1, revision, preferences };
}

const STATUS: {
  device_id: string;
  code: string;
  approved: boolean;
  joined: boolean;
  words: number;
  pending_upload: number;
  pending_apply: number;
  held_deletions: number | null;
  clock_skew: boolean;
  last_success_ms: number | null;
  last_error: string | null;
  budget_wait_until: number | null;
} = {
  device_id: LOCAL_ID,
  code: LOCAL_CODE,
  approved: true,
  joined: true,
  words: 12,
  pending_upload: 1,
  pending_apply: 0,
  held_deletions: null,
  clock_skew: false,
  last_success_ms: Date.UTC(2026, 9, 27, 8, 0),
  last_error: null,
  budget_wait_until: null,
};

const REPORT = {
  captured: 0,
  received_events: 0,
  applied: 0,
  deferred: false,
  published_ops: 0,
  pending_key: false,
  compacted: false,
  rejoined: false,
};

function runValue(status: Partial<typeof STATUS> = {}, nextCheckMs = 1_800_000) {
  return {
    network: true,
    next_check_ms: nextCheckMs,
    status: { ...STATUS, ...status },
    report: REPORT,
  };
}

const DEVICES = {
  enabled: true,
  status: "ready",
  device_id: LOCAL_ID,
  code: LOCAL_CODE,
  devices: [
    {
      device_id: LOCAL_ID,
      name: "书房的 Mac",
      created_at: "2026-10-20T08:00:00Z",
      approved: true,
      current: true,
      code: LOCAL_CODE,
    },
    {
      device_id: OTHER_ID,
      name: "",
      created_at: "2026-10-26T08:00:00Z",
      approved: false,
      current: false,
      code: OTHER_CODE,
    },
    // 同一台设备重复登记的那一条（#10032 之前的版本会留下），只显示一次。
    {
      device_id: OTHER_ID,
      name: "客厅的平板",
      created_at: "2026-10-26T09:00:00Z",
      approved: false,
      current: false,
      code: OTHER_CODE,
    },
  ],
};

type Responder = (action: DictionarySyncAction) => unknown;

/** 宿主的词库同步：按动作答合成的结果，记下每一个请求。 */
function syncHost(overrides: Partial<Record<DictionarySyncAction["operation"], Responder>> = {}) {
  const request = vi.fn(async (action: DictionarySyncAction) => {
    const respond = overrides[action.operation];
    if (respond) return respond(action);
    switch (action.operation) {
      case "run":
        return runValue();
      case "status":
        return STATUS;
      case "devices":
      case "approve":
      case "reject":
        return DEVICES;
      case "remove":
      case "reset":
        return { devices: DEVICES, run: runValue() };
      case "restart":
        return STATUS;
      case "probe":
        return {};
    }
  });
  return { request };
}

function operations(request: ReturnType<typeof syncHost>["request"]) {
  return request.mock.calls.map(([action]) =>
    action.operation === "run" ? `run:${action.trigger}` : action.operation,
  );
}

function renderDictionaryPage(client: Partial<SettingsClient>, preferences: Preferences = base) {
  const save = vi.fn(async (revision: number, next: Preferences) => snapshot(next, revision + 1));
  render(
    <SettingsPage
      initialPage="dictionary"
      client={{
        load: vi.fn().mockResolvedValue(snapshot(preferences)),
        save,
        ...client,
      }}
    />,
  );
  return { save };
}

test("sync results are read strictly and errors have their own words", () => {
  expect(dictionarySyncStatus({ approved: "yes", joined: true })).toBeNull();
  const status = dictionarySyncStatus({ ...STATUS, held_deletions: 60, last_success_ms: null });
  expect(status?.held_deletions).toBe(60);
  expect(status?.last_success_ms).toBeNull();
  // 失败的同步没有 report，error 是错误码，status 照样给出。
  const failed = dictionarySyncRun({
    network: true,
    next_check_ms: 60_000,
    status: STATUS,
    error: "dictionary_sync_unauthorized",
  });
  expect(failed?.error).toBe("dictionary_sync_unauthorized");
  expect(dictionarySyncRun(runValue())?.error).toBeNull();
  expect(dictionarySyncRun({ network: true })).toBeNull();
  expect(dictionarySyncErrorMessage("dictionary_sync_unauthorized")).toBe(
    "网盘拒绝了这个账号或应用密码。",
  );
  // 同一个错误码在「测试连接」和同步时说的是两件事。
  expect(dictionarySyncErrorMessage({ code: "dictionary_sync_unsupported" }, true)).toContain(
    "不支持词库同步需要的文件操作",
  );
  expect(dictionarySyncErrorMessage({ code: "dictionary_sync_unsupported" })).toContain(
    "更新水杉输入法",
  );
  expect(dictionarySyncErrorMessage({ code: "dictionary_sync_rollback" })).toContain(
    "确认网盘现状并恢复同步",
  );
  for (const code of [
    "dictionary_sync_not_configured",
    "invalid_dictionary_sync",
    "dictionary_sync_unavailable",
    "dictionary_sync_rate_limited",
    "dictionary_sync_too_large",
    "dictionary_sync_corrupt",
    "dictionary_sync_storage",
    "clipboard_device_pending",
    "not_found",
    "clipboard_key_conflict",
  ]) {
    expect(dictionarySyncErrorMessage({ code })).not.toBe("同步失败，请稍后再试。");
  }
  expect(dictionarySyncErrorMessage({ code: "something_new" })).toBe("同步失败，请稍后再试。");
  // Rust 给的下一次时间不小于 1 秒，这里另给一个上限。
  expect(dictionarySyncDelay(10)).toBe(1_000);
  expect(dictionarySyncDelay(5_000)).toBe(5_000);
  expect(dictionarySyncDelay(10 * 60 * 60 * 1000)).toBe(30 * 60 * 1000);
});

test("the automatic budget wait is read and said in local time", () => {
  // 旧版本的 Rust 不给这个字段；给了不合法的值也当作没在等。
  expect(dictionarySyncStatus(STATUS)?.budget_wait_until).toBeNull();
  const { budget_wait_until: _, ...older } = STATUS;
  expect(dictionarySyncStatus(older)?.budget_wait_until).toBeNull();
  expect(dictionarySyncStatus({ ...STATUS, budget_wait_until: -1 })?.budget_wait_until).toBeNull();
  expect(
    dictionarySyncStatus({ ...STATUS, budget_wait_until: "soon" })?.budget_wait_until,
  ).toBeNull();
  const until = new Date(2026, 9, 28, 9, 5).getTime();
  expect(dictionarySyncStatus({ ...STATUS, budget_wait_until: until })?.budget_wait_until).toBe(
    until,
  );
  expect(dictionarySyncRun(runValue({ budget_wait_until: until }))?.status.budget_wait_until).toBe(
    until,
  );

  // 按本机时间说；今天之内只写时分，跨了天写上月日；已经过了或没在等时不说。
  const morning = new Date(2026, 9, 28, 8, 40).getTime();
  expect(dictionarySyncBudgetWaitMessage(until, morning)).toBe(
    "额度用完，将于 09:05 继续自动同步；可点「立即同步」。",
  );
  const lateNight = new Date(2026, 9, 27, 23, 50).getTime();
  expect(dictionarySyncBudgetWaitMessage(until, lateNight)).toBe(
    "额度用完，将于 10月28日 09:05 继续自动同步；可点「立即同步」。",
  );
  expect(dictionarySyncBudgetWaitMessage(until, until)).toBeNull();
  expect(dictionarySyncBudgetWaitMessage(until, until + 60_000)).toBeNull();
  expect(dictionarySyncBudgetWaitMessage(null, morning)).toBeNull();
});

test("saving or stopping sync only touches its own key", () => {
  const withClipboard: Preferences = { ...base, cloud_clipboard_webdav: WEBDAV };
  const configured = withDictionarySyncWebDav(withClipboard, WEBDAV);
  expect(configured.dictionary_sync_webdav).toEqual(WEBDAV);
  expect(configured.cloud_clipboard_webdav).toEqual(WEBDAV);
  const cleared = withDictionarySyncWebDav(configured, undefined);
  expect("dictionary_sync_webdav" in cleared).toBe(false);
  expect(cleared).toEqual(withClipboard);
});

test("one device is listed once however many times it registered", () => {
  const { approved, waiting } = dictionarySyncDeviceGroups(DEVICES.devices);
  expect(approved.map((device) => device.device_id)).toEqual([LOCAL_ID]);
  expect(waiting).toHaveLength(1);
  expect(waiting[0].name).toBe("客厅的平板");
  // 任一条已获准就算已获准。
  const merged = dictionarySyncDeviceGroups([
    { ...DEVICES.devices[1], approved: true },
    DEVICES.devices[2],
  ]);
  expect(merged.approved).toHaveLength(1);
  expect(merged.waiting).toHaveLength(0);
});

test("hosts without dictionary sync do not show the section", async () => {
  renderDictionaryPage({});
  await settingsFormReady();
  expect(screen.queryByText(DICTIONARY_SYNC_NOTE)).toBeNull();
  expect(screen.queryByRole("heading", { name: "词库同步" })).toBeNull();
});

test("the drive is saved only after the connection test passes and the draft survives a failure", async () => {
  const host = syncHost({
    probe: vi
      .fn()
      .mockRejectedValueOnce({ code: "dictionary_sync_unsupported" })
      .mockResolvedValue({}),
  });
  const { save } = renderDictionaryPage({ dictionarySync: host });
  await settingsFormReady();
  expect(await screen.findByText(DICTIONARY_SYNC_NOTE)).toBeDefined();
  // 没配置时不同步，也不显示设备。
  expect(host.request).not.toHaveBeenCalled();
  expect(screen.queryByRole("heading", { name: "词库同步设备" })).toBeNull();
  const saveButton = screen.getByRole("button", { name: "保存词库同步设置" }) as HTMLButtonElement;
  const probeButton = screen.getByRole("button", { name: "测试词库同步连接" }) as HTMLButtonElement;
  expect(saveButton.disabled).toBe(true);
  fireEvent.change(screen.getByRole("textbox", { name: "词库同步服务器地址" }), {
    target: { value: "http://dav.example.test/dav/" },
  });
  expect(screen.getByText(/WebDAV 服务器必须使用 HTTPS/)).toBeDefined();
  fireEvent.change(screen.getByRole("textbox", { name: "词库同步服务器地址" }), {
    target: { value: ` ${WEBDAV.url} ` },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "词库同步账号" }), {
    target: { value: WEBDAV.username },
  });
  const password = screen.getByLabelText("词库同步应用密码") as HTMLInputElement;
  expect(password.type).toBe("password");
  fireEvent.change(password, { target: { value: WEBDAV.password } });
  await waitFor(() => expect(saveButton.disabled).toBe(false));
  saveSettingsNow();
  expect(save).not.toHaveBeenCalled();

  // 网盘没通过：不保存，说明原因，草稿留着。
  fireEvent.click(saveButton);
  expect(await screen.findByText(/不支持词库同步需要的文件操作/)).toBeDefined();
  saveSettingsNow();
  expect(save).not.toHaveBeenCalled();
  expect(
    (screen.getByRole("textbox", { name: "词库同步服务器地址" }) as HTMLInputElement).value.trim(),
  ).toBe(WEBDAV.url);
  expect(host.request).toHaveBeenLastCalledWith({ operation: "probe", ...WEBDAV });

  // 只测试连接：通过，仍不保存。
  fireEvent.click(probeButton);
  expect(await screen.findByText("连接正常，可以保存。")).toBeDefined();
  saveSettingsNow();
  expect(save).not.toHaveBeenCalled();

  // 通过后保存：只写词库同步，保存之后以 `opened` 同步一次。
  fireEvent.click(saveButton);
  await waitFor(() =>
    expect(operations(host.request).filter((op) => op === "probe")).toHaveLength(3),
  );
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(save.mock.calls[0][1].dictionary_sync_webdav).toEqual(WEBDAV);
  expect("cloud_clipboard_webdav" in save.mock.calls[0][1]).toBe(false);
  await waitFor(() => expect(operations(host.request)).toContain("run:opened"));
  expect(await screen.findByRole("heading", { name: "词库同步设备" })).toBeDefined();
});

test("a saved app password is never echoed and blank keeps it", async () => {
  const host = syncHost();
  const { save } = renderDictionaryPage(
    { dictionarySync: host },
    { ...base, dictionary_sync_webdav: WEBDAV },
  );
  await settingsFormReady();
  const password = (await screen.findByLabelText("词库同步应用密码")) as HTMLInputElement;
  expect(password.value).toBe("");
  expect(password.placeholder).toBe("留空以保留已保存的应用密码");
  expect(document.body.textContent).not.toContain(WEBDAV.password);
  const address = screen.getByRole("textbox", { name: "词库同步服务器地址" }) as HTMLInputElement;
  await waitFor(() => expect(address.value).toBe(WEBDAV.url));
  fireEvent.change(address, { target: { value: "https://dav.example.test/dav/other/" } });
  const saveButton = screen.getByRole("button", { name: "保存词库同步设置" }) as HTMLButtonElement;
  await waitFor(() => expect(saveButton.matches(":disabled")).toBe(false));
  fireEvent.click(saveButton);
  await waitFor(() => expect(operations(host.request)).toContain("probe"));
  const probe = host.request.mock.calls.find(([action]) => action.operation === "probe")?.[0];
  expect(probe).toEqual({
    operation: "probe",
    ...WEBDAV,
    url: "https://dav.example.test/dav/other/",
  });
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(save.mock.calls[0][1].dictionary_sync_webdav).toEqual({
    ...WEBDAV,
    url: "https://dav.example.test/dav/other/",
  });
});

test("a configured host syncs on open, follows next_check_ms and syncs on demand", async () => {
  let runs = 0;
  const host = syncHost({
    // 第一次让宿主 1 秒后再来，之后照常半小时。
    run: () => runValue({}, runs++ === 0 ? 1_000 : 1_800_000),
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  await waitFor(() => expect(operations(host.request)[0]).toBe("run:opened"));
  expect(await screen.findByText(/上次同步：/)).toBeDefined();
  expect(screen.getByText(/共享词库有 12 个词条；待发送 1 项/)).toBeDefined();
  await waitFor(() => expect(operations(host.request)).toContain("run:periodic"), {
    timeout: 3_000,
  });
  fireEvent.click(screen.getByRole("button", { name: "立即同步" }));
  await waitFor(() => expect(operations(host.request)).toContain("run:manual"));
  expect(await screen.findByText("已同步。")).toBeDefined();
});

test("a failed sync says why in Chinese", async () => {
  const host = syncHost({
    run: () => ({
      network: true,
      next_check_ms: 60_000,
      status: { ...STATUS, last_error: "dictionary_sync_unauthorized" },
      error: "dictionary_sync_unauthorized",
    }),
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  expect(await screen.findByText("网盘拒绝了这个账号或应用密码。")).toBeDefined();
});

test("a sync waiting for its request budget says when it resumes and keeps the error", async () => {
  const until = Date.now() + 20 * 60 * 1000;
  const waiting = {
    ...STATUS,
    last_error: "dictionary_sync_rate_limited",
    budget_wait_until: until,
  };
  const host = syncHost({
    run: () => ({
      network: false,
      next_check_ms: 1_200_000,
      status: waiting,
      error: "dictionary_sync_rate_limited",
    }),
    // 窗口回到前台时重读的概况和同步结果读的是同一份状态，额度等待时间也在里面；默认的 `STATUS` 没有它，会在同步结果之后把提示冲掉，断言就只能赶在两次响应之间的空档里查到。
    status: () => waiting,
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  const status = await screen.findByLabelText("词库同步状态");
  const message = dictionarySyncBudgetWaitMessage(until);
  expect(message).not.toBeNull();
  expect(await within(status).findByText(message!)).toBeDefined();
  // 不顶掉错误，也不顶掉「上次同步」。
  expect(within(status).getByText(/网盘暂时限制了请求次数/)).toBeDefined();
  expect(within(status).getByText(/上次同步：/)).toBeDefined();
});

test("a budget wait that has already passed is not shown", async () => {
  const host = syncHost({ run: () => runValue({ budget_wait_until: Date.now() - 1_000 }) });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  const status = await screen.findByLabelText("词库同步状态");
  await within(status).findByText(/上次同步：/);
  expect(within(status).queryByText(/额度用完/)).toBeNull();
});

test("words changed through settings are recorded as a local change", async () => {
  const notify = vi.fn();
  const edit = vi.fn().mockResolvedValue(undefined);
  const list = vi.fn().mockResolvedValue({ entries: [], has_more: false });
  const dictionary = { list, edit } as unknown as DictionaryClient;
  const host = syncHost();
  const wrapped = withDictionaryChangeNotices(
    { load: vi.fn(), save: vi.fn(), dictionary, dictionarySync: host },
    notify,
  );
  await wrapped.dictionary!.list(0, 50, "pinyin", "");
  expect(notify).not.toHaveBeenCalled();
  await wrapped.dictionary!.edit(
    { key: "shui'shan", value: "水杉", weight: 10 } as never,
    null,
    "ui-remove-1",
  );
  expect(edit).toHaveBeenCalledTimes(1);
  expect(notify).toHaveBeenCalledTimes(1);
  // 写失败时不记。
  edit.mockRejectedValueOnce(new Error("busy"));
  await expect(wrapped.dictionary!.edit({} as never, null, "ui-remove-2")).rejects.toThrow();
  expect(notify).toHaveBeenCalledTimes(1);
  // 没有词库同步的宿主拿到的还是同一个客户端。
  const plain = { load: vi.fn(), save: vi.fn(), dictionary };
  expect(withDictionaryChangeNotices(plain, notify)).toBe(plain);

  // 通知到了词库同步，连续几次只排一次 `local_change`。
  const localChanges = createDictionaryLocalChanges();
  renderHook(() =>
    useDictionarySync({ client: { dictionarySync: host }, saved: WEBDAV, localChanges }),
  );
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened"]));
  act(() => {
    localChanges.notify();
    localChanges.notify();
  });
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened", "run:local_change"]));
});

test("words the host wrote itself are recorded as a local change only while sync is offered", async () => {
  const localChanges = createDictionaryLocalChanges();
  const host = syncHost();
  const { rerender, unmount } = renderHook(
    ({ saved }) => useDictionarySync({ client: { dictionarySync: host }, saved, localChanges }),
    { initialProps: { saved: WEBDAV as typeof WEBDAV | undefined } },
  );
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened"]));
  // 鸿蒙设置应用把键盘的词写进了自己的词库：连着派发几次也只排一次。
  act(() => {
    window.dispatchEvent(new Event(HOST_USER_WORDS_CHANGED_EVENT));
    window.dispatchEvent(new Event(HOST_USER_WORDS_CHANGED_EVENT));
  });
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened", "run:local_change"]));
  // 停用网盘以后不再听。
  rerender({ saved: undefined });
  act(() => {
    window.dispatchEvent(new Event(HOST_USER_WORDS_CHANGED_EVENT));
  });
  unmount();
  window.dispatchEvent(new Event(HOST_USER_WORDS_CHANGED_EVENT));
  await new Promise((resolve) => setTimeout(resolve, 20));
  expect(operations(host.request)).toEqual(["run:opened", "run:local_change"]);

  // 宿主不提供词库同步（鸿蒙没有隐私政策的授权时就是这样）：事件不会让页面发出任何请求。
  const unconsented = vi.fn();
  renderHook(() =>
    useDictionarySync({ client: { dictionarySync: undefined }, saved: WEBDAV, localChanges }),
  );
  const listener = localChanges.subscribe(unconsented);
  act(() => {
    window.dispatchEvent(new Event(HOST_USER_WORDS_CHANGED_EVENT));
  });
  listener();
  expect(unconsented).not.toHaveBeenCalled();
});

test("a sync run that wrote remote words into this device's dictionary reloads the word list", async () => {
  expect(dictionarySyncRun({ ...runValue(), report: { ...REPORT, applied: 3 } })?.applied).toBe(3);
  expect(
    dictionarySyncRun({ network: true, next_check_ms: 60_000, status: STATUS, error: "x" })
      ?.applied,
  ).toBe(0);

  // 网盘来的词写进了本机词库之后，词库页重读「我的词条」；什么也没写进来时不重读。
  const localChanges = createDictionaryLocalChanges();
  const onWordsApplied = vi.fn();
  let applied = 0;
  const host = syncHost({
    run: () => ({ ...runValue(), report: { ...REPORT, applied } }),
    remove: () => ({
      devices: DEVICES,
      run: { ...runValue(), report: { ...REPORT, applied: 1 } },
    }),
  });
  const { result } = renderHook(() =>
    useDictionarySync({
      client: { dictionarySync: host },
      saved: WEBDAV,
      localChanges,
      onWordsApplied,
    }),
  );
  await waitFor(() => expect(result.current?.status).not.toBeNull());
  expect(onWordsApplied).not.toHaveBeenCalled();
  applied = 2;
  await act(async () => result.current!.syncNow());
  expect(onWordsApplied).toHaveBeenCalledTimes(1);
  // 移除设备带回的那一次同步也算。
  await act(async () => result.current!.remove(OTHER_ID));
  expect(onWordsApplied).toHaveBeenCalledTimes(2);

  // 整个设置页：在「管理词库」里「立即同步」写进了一个词，回到「查找词库」不用换类型或搜索就看得到它。
  cleanup();
  const word = { kind: "quick_phrase" as const, key: "wpzc", value: "网盘来的合成词", weight: 1 };
  let synced = false;
  const list = vi.fn(async () => ({ entries: synced ? [word] : [], has_more: false }));
  const page = syncHost({
    run: (action) => {
      if (action.operation === "run" && action.trigger === "manual") synced = true;
      return { ...runValue(), report: { ...REPORT, applied: synced ? 1 : 0 } };
    },
  });
  renderDictionaryPage(
    { dictionarySync: page, dictionary: { list, edit: vi.fn() } as unknown as DictionaryClient },
    { ...base, dictionary_sync_webdav: WEBDAV },
  );
  await settingsFormReady();
  await waitFor(() => expect(list).toHaveBeenCalled());
  await waitFor(() => expect(operations(page.request)).toContain("run:opened"));
  expect(screen.queryByText("网盘来的合成词")).toBeNull();
  const calls = list.mock.calls.length;
  fireEvent.click(screen.getByRole("tab", { name: "管理词库" }));
  fireEvent.click(await screen.findByRole("button", { name: "立即同步" }));
  await waitFor(() => expect(list.mock.calls.length).toBeGreaterThan(calls));
  fireEvent.click(screen.getByRole("tab", { name: "查找词库" }));
  expect(await screen.findByText("网盘来的合成词")).toBeDefined();
});

test("returning to the window reads the status the input method's background sync left", async () => {
  const localChanges = createDictionaryLocalChanges();
  // 输入法进程在后台把待发送的那一个词发了出去：本页读到的概况随之变了。
  let background = STATUS;
  const host = syncHost({ status: () => background });
  const { result, unmount } = renderHook(() =>
    useDictionarySync({ client: { dictionarySync: host }, saved: WEBDAV, localChanges }),
  );
  await waitFor(() => expect(result.current?.status?.pending_upload).toBe(1));
  background = { ...STATUS, pending_upload: 0, last_success_ms: Date.UTC(2026, 9, 27, 8, 30) };
  act(() => {
    window.dispatchEvent(new Event("focus"));
  });
  await waitFor(() => expect(result.current?.status?.pending_upload).toBe(0));
  expect(result.current?.status?.last_success_ms).toBe(Date.UTC(2026, 9, 27, 8, 30));
  expect(operations(host.request)).toEqual(["run:opened", "status"]);
  // 卸载以后不再读。
  unmount();
  window.dispatchEvent(new Event("focus"));
  await new Promise((resolve) => setTimeout(resolve, 20));
  expect(operations(host.request)).toEqual(["run:opened", "status"]);
});

test("changing the saved connection releases a previous manual action's busy state", async () => {
  let finishManual!: (value: ReturnType<typeof runValue>) => void;
  const manual = new Promise<ReturnType<typeof runValue>>((resolve) => {
    finishManual = resolve;
  });
  const host = syncHost({
    run: (action) =>
      action.operation === "run" && action.trigger === "manual" ? manual : runValue(),
  });
  const localChanges = createDictionaryLocalChanges();
  const { result, rerender } = renderHook(
    ({ saved }: { saved: typeof WEBDAV | undefined }) =>
      useDictionarySync({ client: { dictionarySync: host }, saved, localChanges }),
    { initialProps: { saved: WEBDAV as typeof WEBDAV | undefined } },
  );
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened"]));
  let action!: Promise<void>;
  act(() => {
    action = result.current!.syncNow();
  });
  await waitFor(() => expect(result.current!.syncing).toBe(true));
  expect(result.current!.busy).toBe(true);
  rerender({ saved: undefined });
  await act(async () => {
    finishManual(runValue());
    await action;
  });
  rerender({ saved: WEBDAV });
  await waitFor(() =>
    expect(operations(host.request)).toEqual(["run:opened", "run:manual", "run:opened"]),
  );
  await waitFor(() => expect(result.current!.syncing).toBe(false));
  expect(result.current!.busy).toBe(false);
});

test("this device's code is shown while it waits, and it cannot approve others yet", async () => {
  const host = syncHost({
    run: () => runValue({ approved: false }),
    status: () => ({ ...STATUS, approved: false }),
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  const waiting = await screen.findByLabelText("本设备等待允许");
  expect(within(waiting).getByLabelText("本设备核对码").textContent).toBe(LOCAL_CODE);
  // 已经登记过的设备打开页面就取一次设备列表；还没获准时手里没有密钥，只列出设备。
  await waitFor(() => expect(operations(host.request)).toContain("devices"));
  expect(await screen.findByText("书房的 Mac（本机）")).toBeDefined();
  expect(screen.queryByRole("button", { name: "允许 客厅的平板" })).toBeNull();
  expect(screen.getByRole("button", { name: "重置密钥" })).toBeDefined();
});

test("waiting devices are listed once and approved after comparing codes", async () => {
  const host = syncHost();
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  expect(await screen.findByText(/有 1 台设备请求同步词库/)).toBeDefined();
  expect(screen.queryByLabelText("本设备等待允许")).toBeNull();
  // 重复登记的设备只有一条。
  expect(screen.getAllByLabelText("客厅的平板 的核对码")).toHaveLength(1);
  expect(screen.getByLabelText("客厅的平板 的核对码").textContent).toBe(OTHER_CODE);
  expect(screen.getByText("书房的 Mac（本机）")).toBeDefined();
  expect(screen.queryByRole("button", { name: "移除 书房的 Mac" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "允许 客厅的平板" }));
  await waitFor(() =>
    expect(host.request).toHaveBeenCalledWith({ operation: "approve", device_id: OTHER_ID }),
  );
  fireEvent.click(screen.getByRole("button", { name: "拒绝 客厅的平板" }));
  await waitFor(() =>
    expect(host.request).toHaveBeenCalledWith({ operation: "reject", device_id: OTHER_ID }),
  );
});

test("removing a device asks first and rewrites the shared dictionary", async () => {
  const approved = {
    ...DEVICES,
    devices: [DEVICES.devices[0], { ...DEVICES.devices[2], approved: true }],
  };
  const host = syncHost({
    devices: () => approved,
    remove: () => ({ devices: { ...approved, devices: [DEVICES.devices[0]] }, run: runValue() }),
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "移除 客厅的平板" }));
  await answerConfirm("cancel");
  expect(operations(host.request)).not.toContain("remove");
  fireEvent.click(screen.getByRole("button", { name: "移除 客厅的平板" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(host.request).toHaveBeenCalledWith({ operation: "remove", device_id: OTHER_ID }),
  );
  await waitFor(() => expect(screen.queryByRole("button", { name: "移除 客厅的平板" })).toBeNull());
});

test("a held mass deletion waits for the user's choice", async () => {
  const host = syncHost({
    run: () => runValue({ held_deletions: 80 }),
    status: () => ({ ...STATUS, held_deletions: 80 }),
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  const held = await screen.findByLabelText("被挡下的删除");
  expect(held.textContent).toContain("少了 80 个词条");
  fireEvent.click(within(held).getByRole("button", { name: "从网盘恢复" }));
  await waitFor(() =>
    expect(host.request).toHaveBeenCalledWith({
      operation: "run",
      trigger: "manual",
      restore_deletions: true,
    }),
  );
  fireEvent.click(within(held).getByRole("button", { name: "同步删除" }));
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(host.request).toHaveBeenCalledWith({
      operation: "run",
      trigger: "manual",
      confirm_deletions: true,
    }),
  );
});

test("resetting the key and restarting from this device both ask first", async () => {
  const host = syncHost();
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "重置密钥" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(operations(host.request)).toContain("reset"));
  fireEvent.click(screen.getByRole("button", { name: "从这台设备重新开始" }));
  await answerConfirm("cancel");
  expect(operations(host.request)).not.toContain("restart");
  fireEvent.click(screen.getByRole("button", { name: "从这台设备重新开始" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(operations(host.request)).toContain("restart"));
  expect(await screen.findByText(/已以本机的词条重新开始/)).toBeDefined();
});

test("stopping sync clears the drive after confirmation", async () => {
  const host = syncHost();
  const { save } = renderDictionaryPage(
    { dictionarySync: host },
    { ...base, dictionary_sync_webdav: WEBDAV },
  );
  await settingsFormReady();
  fireEvent.click(await screen.findByRole("button", { name: "停止同步" }));
  await answerConfirm("confirm");
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect("dictionary_sync_webdav" in save.mock.calls[0][1]).toBe(false);
});

test("the HarmonyOS phone's dictionary page offers sync through the host bridge", async () => {
  // 鸿蒙设置应用（apps/harmony）在同意隐私政策后才提供 `dictionarySync`，请求经 ArkTS 桥和 NAPI 交给 Rust。
  const host = syncHost();
  const { save } = renderDictionaryPage({
    host: testHost({ platform: "harmony" }),
    dictionarySync: host,
  });
  await settingsFormReady();
  expect(await screen.findByText(DICTIONARY_SYNC_NOTE)).toBeDefined();
  fireEvent.change(screen.getByRole("textbox", { name: "词库同步服务器地址" }), {
    target: { value: WEBDAV.url },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "词库同步账号" }), {
    target: { value: WEBDAV.username },
  });
  fireEvent.change(screen.getByLabelText("词库同步应用密码"), {
    target: { value: WEBDAV.password },
  });
  const saveButton = screen.getByRole("button", { name: "保存词库同步设置" }) as HTMLButtonElement;
  await waitFor(() => expect(saveButton.disabled).toBe(false));
  fireEvent.click(saveButton);
  await waitFor(() => expect(host.request).toHaveBeenCalledWith({ operation: "probe", ...WEBDAV }));
  saveSettingsNow();
  await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
  expect(save.mock.calls[0][1].dictionary_sync_webdav).toEqual(WEBDAV);
  await waitFor(() => expect(operations(host.request)).toContain("run:opened"));
});

test("changing the drive cancels queued device actions and clears busy state", async () => {
  let finishOpened!: (value: unknown) => void;
  const host = syncHost({
    run: vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finishOpened = resolve;
          }),
      )
      .mockImplementation(() => runValue()),
  });
  const localChanges = createDictionaryLocalChanges();
  const { result, rerender } = renderHook(
    ({ saved }) => useDictionarySync({ client: { dictionarySync: host }, saved, localChanges }),
    { initialProps: { saved: WEBDAV } },
  );
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened"]));
  let pending!: Promise<void>;
  act(() => {
    pending = result.current!.reset();
  });
  expect(result.current!.busy).toBe(true);
  rerender({ saved: { ...WEBDAV, url: "https://other.example.test/dav/" } });
  expect(result.current!.busy).toBe(false);
  await act(async () => {
    finishOpened(runValue());
    await pending;
  });
  await waitFor(() => expect(operations(host.request)).toEqual(["run:opened", "run:opened"]));
  expect(result.current!.busy).toBe(false);
  expect(result.current!.syncing).toBe(false);
});

test("stopping the drive during an automatic request clears syncing", async () => {
  let finishOpened!: (value: unknown) => void;
  const host = syncHost({
    run: () =>
      new Promise((resolve) => {
        finishOpened = resolve;
      }),
  });
  const localChanges = createDictionaryLocalChanges();
  const { result, rerender } = renderHook(
    ({ saved }: { saved: typeof WEBDAV | undefined }) =>
      useDictionarySync({ client: { dictionarySync: host }, saved, localChanges }),
    { initialProps: { saved: WEBDAV as typeof WEBDAV | undefined } },
  );
  await waitFor(() => expect(result.current!.syncing).toBe(true));
  rerender({ saved: undefined });
  expect(result.current!.syncing).toBe(false);
  await act(async () => {
    finishOpened(runValue());
  });
  expect(result.current!.status).toBeNull();
  expect(result.current!.syncing).toBe(false);
});

test("recovering after remote cleanup requires confirmation and uses the saved connection", async () => {
  const host = syncHost({
    run: () => ({ ...runValue(), report: undefined, error: "dictionary_sync_rollback" }),
  });
  renderDictionaryPage({ dictionarySync: host }, { ...base, dictionary_sync_webdav: WEBDAV });
  await settingsFormReady();
  const recover = await screen.findByRole("button", { name: "确认网盘现状并恢复同步" });
  fireEvent.click(recover);
  expect(operations(host.request)).not.toContain("probe");
  expect(await screen.findByText(/未上传的删除可能从网盘重新合并回来/)).toBeDefined();
  await answerConfirm("confirm");
  await waitFor(() =>
    expect(host.request).toHaveBeenCalledWith({
      operation: "probe",
      ...WEBDAV,
      trust_remote: true,
    }),
  );
  await waitFor(() => expect(operations(host.request)).toContain("run:manual"));
});

test("changing connection while recovery confirmation is open cancels the old recovery", async () => {
  const host = syncHost();
  const localChanges = createDictionaryLocalChanges();
  const { result, rerender } = renderHook(
    ({ saved }) => useDictionarySync({ client: { dictionarySync: host }, saved, localChanges }),
    { initialProps: { saved: WEBDAV } },
  );
  await waitFor(() => expect(operations(host.request)).toContain("run:opened"));
  let answer!: (value: boolean) => void;
  const confirmation = new Promise<boolean>((resolve) => {
    answer = resolve;
  });
  let recovery!: Promise<void>;
  act(() => {
    recovery = result.current!.trustRemote(() => confirmation);
  });
  rerender({ saved: { ...WEBDAV, url: "https://dav.example.test/changed/" } });
  await waitFor(() =>
    expect(operations(host.request).filter((op) => op === "run:opened")).toHaveLength(2),
  );
  await act(async () => {
    answer(true);
    await recovery;
  });
  expect(operations(host.request)).not.toContain("probe");
  expect(operations(host.request)).not.toContain("run:manual");
});
