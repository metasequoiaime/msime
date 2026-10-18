// @vitest-environment jsdom
import { findVisibleText, selectDictionaryTab, settingsFormReady } from "../support/settings-form";
import { answerConfirm } from "../support/confirm";
import { testHost } from "../support/host";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 2,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

const resetNotice = "已清除所有学习数据；输入方案和设置保持不变。";

function desktopClient(fields: Partial<SettingsClient> = {}): SettingsClient {
  return {
    load: vi.fn().mockResolvedValue(snapshot),
    save: vi.fn(),
    host: testHost({ platform: "windows" }),
    dictionary: {
      list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
      edit: vi.fn(),
      export: vi
        .fn()
        .mockResolvedValue({ text: "synthetic phrase\tfixture\t10\n", has_more: false }),
    },
    ...fields,
  };
}

async function openDictionary() {
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(词库|词库管理)$/ }));
}

// 清除学习数据的按钮在「管理词库」页末，结果画在页顶的话，用户滚到页末点完按钮什么也看不到；结果要画在「学习数据」组里、按钮旁边。
test("the learning data reset result is shown in the 学习数据 group beside its button", async () => {
  const resetLearnedData = vi.fn().mockResolvedValue(undefined);
  render(<SettingsPage client={desktopClient({ resetLearnedData })} />);
  await openDictionary();
  selectDictionaryTab("管理词库");
  const group = await screen.findByRole("region", { name: "学习数据" });
  fireEvent.click(within(group).getByRole("button", { name: "清除全部学习数据" }));
  await answerConfirm("confirm");
  await waitFor(() => expect(resetLearnedData).toHaveBeenCalledOnce());
  const notice = await findVisibleText(resetNotice);
  expect(group.contains(notice)).toBe(true);
  // 只画这一份，页顶不再重复。
  expect(screen.getAllByText(resetNotice)).toHaveLength(1);
});

test("a failed learning data reset shows its error in the 学习数据 group", async () => {
  const resetLearnedData = vi.fn().mockRejectedValue(new Error("synthetic"));
  render(<SettingsPage client={desktopClient({ resetLearnedData })} />);
  await openDictionary();
  selectDictionaryTab("管理词库");
  const group = await screen.findByRole("region", { name: "学习数据" });
  fireEvent.click(within(group).getByRole("button", { name: "清除全部学习数据" }));
  await answerConfirm("confirm");
  const alert = await within(group).findByRole("alert");
  expect(alert.textContent).toContain("清除学习数据失败");
  expect(screen.getAllByRole("alert")).toHaveLength(1);
});

// 导入后重新读取的列表会带回等待同步数、快照错误和失败记录；它们要在用户导入的「管理词库」标签上看得到，重试按钮只有一份。
test("dictionary state reported after an import is visible on 管理词库 without duplicated controls", async () => {
  const failure = { request_id: "synthetic-request", label: "synthetic word", error: "busy" };
  let imported = false;
  const list = vi.fn().mockImplementation(async () =>
    imported
      ? {
          entries: [],
          has_more: false,
          pending_count: 2,
          failed_requests: [failure],
          snapshot_error: "synthetic snapshot error",
        }
      : { entries: [], has_more: false },
  );
  const client = desktopClient({
    dictionary: {
      list,
      edit: vi.fn(),
      import: vi.fn().mockImplementation(async () => {
        imported = true;
        return { applied: 1 };
      }),
      retry: vi.fn().mockResolvedValue(undefined),
      dismissFailure: vi.fn().mockResolvedValue(undefined),
    },
  });
  render(<SettingsPage client={client} />);
  await openDictionary();
  const kind = screen.getByRole<HTMLSelectElement>("combobox", { name: "本地词库类型" });
  await waitFor(() => expect(kind.disabled).toBe(false));
  selectDictionaryTab("管理词库");
  const transfer = screen.getByRole("region", { name: "导入与导出" });
  fireEvent.change(within(transfer).getByLabelText("导入"), {
    target: { files: [new File(["synthetic\tfixture\n"], "words.txt")] },
  });
  await findVisibleText("2 项等待键盘同步。打开水杉键盘后会在空闲时逐条生效。");
  await findVisibleText("synthetic snapshot error");
  await findVisibleText("有 1 项词库请求同步失败，可以重试或移除失败记录。");
  // 可见的重试、移除按钮各一个，隐藏的标签里也没有第二份。
  expect(screen.getAllByRole("button", { name: "重试" })).toHaveLength(1);
  expect(screen.getAllByRole("button", { name: "重试", hidden: true })).toHaveLength(1);
  expect(screen.getAllByRole("button", { name: "移除记录", hidden: true })).toHaveLength(1);
  // 这是词库本身的状态，不是某一次操作的结果，切回「查找词库」仍然看得到。
  selectDictionaryTab("查词库");
  await findVisibleText("2 项等待键盘同步。打开水杉键盘后会在空闲时逐条生效。");
});

// 一个标签上的操作结果不跟到另一个标签：查询失败的提示不该留在「管理词库」的导入导出上面。
test("a lookup error is not carried to 管理词库", async () => {
  const client = desktopClient({
    dictionary: { list: vi.fn().mockRejectedValue(new Error("synthetic")), edit: vi.fn() },
  });
  render(<SettingsPage client={client} />);
  await openDictionary();
  await findVisibleText("无法读取词库。");
  selectDictionaryTab("管理词库");
  expect(screen.queryByText("无法读取词库。")).toBeNull();
});

test("an export notice from 管理词库 is not carried to 查找词库 and does not come back", async () => {
  render(<SettingsPage client={desktopClient({ saveExport: vi.fn().mockResolvedValue(null) })} />);
  await openDictionary();
  selectDictionaryTab("管理词库");
  fireEvent.click(await screen.findByRole("button", { name: "导出当前类型" }));
  await findVisibleText("已取消导出。");
  selectDictionaryTab("查词库");
  expect(screen.queryByText("已取消导出。")).toBeNull();
  selectDictionaryTab("管理词库");
  expect(screen.queryByText("已取消导出。")).toBeNull();
});

// 宿主的本地词库晚于页面出现时，用户还没点过标签，就落到默认的「查找词库」上。
test("the dictionary page selects 查找词库 once the local dictionary arrives", async () => {
  const withoutDictionary = desktopClient({
    dictionary: undefined,
    openCloudDictionary: vi.fn().mockResolvedValue(undefined),
  });
  const { rerender } = render(<SettingsPage client={withoutDictionary} />);
  await openDictionary();
  expect(screen.queryByRole("tablist", { name: "词库设置分类" })).toBeNull();
  rerender(
    <SettingsPage
      client={{
        ...withoutDictionary,
        dictionary: {
          list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
          edit: vi.fn(),
        },
      }}
    />,
  );
  const tabs = await screen.findByRole("tablist", { name: "词库设置分类" });
  expect(within(tabs).getByRole("tab", { name: "查找词库" }).getAttribute("aria-selected")).toBe(
    "true",
  );
  // `getByRole` 跳过隐藏的元素：查询框在当前标签上看得到。
  expect(screen.getByRole("searchbox", { name: "词条或编码" })).toBeTruthy();
});
