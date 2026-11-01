// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { SettingsPage, type SettingsClient, type Snapshot } from "@msime/ui";
import type { DictionaryClient } from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import { ResourceDictionaryLookup } from "../../../../packages/ui/src/settings/resource-dictionary-lookup";
import { settingsFormReady } from "../support/settings-form";

const empty = { entries: [], has_more: false, available: true };
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("语言资源与个人词库都出现在查询下拉，切到日语即列出全部词条", async () => {
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
    },
  };
  const browse = vi.fn().mockImplementation(async (_offset, _limit, kind) =>
    kind === "japanese"
      ? {
          entries: [{ kind, key: "にほん", value: "日本", weight: -10, source: "bundled" }],
          has_more: false,
          available: true,
        }
      : empty,
  );
  const client: SettingsClient = {
    load: async () => snapshot,
    save: vi.fn(),
    dictionary: { browse, list: vi.fn(), edit: vi.fn() },
  };
  render(<SettingsPage client={client} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(词库|词库管理)$/ }));
  const select = screen.getByLabelText("本地词库类型");
  expect(
    within(select)
      .getAllByRole("option")
      .map((option) => (option as HTMLOptionElement).value),
  ).toEqual([
    "pinyin",
    "wubi",
    "wubi98",
    "english",
    "quick_phrase",
    "cantonese",
    "resource:japanese",
    "resource:korean_hanja",
    "resource:cantonese",
    "resource:zhuyin",
    "resource:stroke",
  ]);
  fireEvent.change(select, { target: { value: "resource:japanese" } });
  expect(await screen.findByText("日本")).toBeTruthy();
  expect(browse).toHaveBeenCalledWith(0, 100, "japanese", "");
  expect(screen.queryByRole("button", { name: "新增词条" })).toBeNull();
  expect(screen.queryByRole("button", { name: "编辑" })).toBeNull();
  expect(screen.queryByRole("button", { name: "删除" })).toBeNull();
  expect(screen.getByText("读音")).toBeTruthy();
  expect(screen.queryByText(/成本/)).toBeNull();
  expect(screen.queryByText("-10")).toBeNull();
});

test("资源查询防抖、分页，条件变化后忽略旧响应", async () => {
  vi.useFakeTimers();
  let finishOld: (value: typeof empty) => void = () => {};
  const browse = vi.fn().mockResolvedValue(empty);
  browse.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finishOld = resolve;
      }),
  );
  const dictionary: DictionaryClient = { browse, list: vi.fn(), edit: vi.fn() };
  render(<ResourceDictionaryLookup dictionary={dictionary} kind="japanese" />);
  await act(() => vi.advanceTimersByTimeAsync(280));
  const input = screen.getByRole("searchbox", { name: "词条或编码" });
  fireEvent.change(input, { target: { value: "日" } });
  await act(() => vi.advanceTimersByTimeAsync(100));
  fireEvent.change(input, { target: { value: "日本" } });
  await act(() => vi.advanceTimersByTimeAsync(279));
  expect(browse).toHaveBeenCalledTimes(1);
  await act(async () => {
    finishOld({ ...empty, has_more: true });
  });
  expect((screen.getByRole("button", { name: "下一页" }) as HTMLButtonElement).disabled).toBe(true);
  browse.mockResolvedValue({
    entries: [{ kind: "japanese", key: "にほん", value: "日本", weight: 10, source: "bundled" }],
    has_more: true,
    available: true,
  });
  await act(() => vi.advanceTimersByTimeAsync(1));
  expect(browse).toHaveBeenLastCalledWith(0, 100, "japanese", "日本");
  expect(screen.queryByRole("button", { name: "搜索" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  await act(() => vi.advanceTimersByTimeAsync(280));
  expect(browse).toHaveBeenLastCalledWith(100, 100, "japanese", "日本");
  fireEvent.change(input, { target: { value: "仮名" } });
  await act(() => vi.advanceTimersByTimeAsync(280));
  expect(browse).toHaveBeenLastCalledWith(0, 100, "japanese", "仮名");
});

test("未安装的词库显示下载说明，区别于空结果", async () => {
  const dictionary = {
    list: vi.fn(),
    edit: vi.fn(),
    browse: vi.fn().mockResolvedValue({ ...empty, available: false }),
  };
  render(<ResourceDictionaryLookup dictionary={dictionary} kind="cantonese" />);
  await waitFor(() => expect(screen.getAllByText(/词库尚未安装/).length).toBeGreaterThan(0));
  expect(screen.queryByText("未找到匹配词条")).toBeNull();
});

test("粤语的个人词库和基础词库在下拉里分开：个人粤语词可编辑，基础词库走只读浏览", async () => {
  const snapshot: Snapshot = {
    format_version: 1,
    revision: 1,
    preferences: {
      scheme: "quanpin",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 5,
      learning: true,
      chinese_punctuation: true,
    },
  };
  const browse = vi.fn().mockResolvedValue({
    entries: [
      { kind: "cantonese", key: "nei hou", value: "你好", weight: 21493, source: "bundled" },
    ],
    has_more: false,
    available: true,
  });
  const list = vi.fn().mockImplementation(async (_offset, _limit, kind) => ({
    entries:
      kind === "cantonese"
        ? [{ kind: "cantonese", key: "nei hou", value: "禮好", weight: 100000, source: "user" }]
        : [],
    has_more: false,
  }));
  const client: SettingsClient = {
    load: async () => snapshot,
    save: vi.fn(),
    dictionary: { browse, list, edit: vi.fn() },
  };
  render(<SettingsPage client={client} />);
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(词库|词库管理)$/ }));
  const select = screen.getByLabelText("本地词库类型");
  fireEvent.change(select, { target: { value: "cantonese" } });
  expect(await screen.findByText("禮好")).toBeTruthy();
  expect(list).toHaveBeenLastCalledWith(0, 100, "cantonese", "");
  expect(browse).not.toHaveBeenCalledWith(
    expect.anything(),
    expect.anything(),
    "cantonese",
    expect.anything(),
  );
  expect(screen.getByRole("button", { name: "编辑" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "新增词条" }));
  expect(screen.getByText("粤拼，每字一个音节，用空格分隔，如 nei hou")).toBeTruthy();
  expect((screen.getByLabelText(/权重/) as HTMLInputElement).value).toBe("100000");

  fireEvent.change(select, { target: { value: "resource:cantonese" } });
  expect(await screen.findByText("你好")).toBeTruthy();
  expect(browse).toHaveBeenCalledWith(0, 100, "cantonese", "");
  expect(screen.queryByRole("button", { name: "编辑" })).toBeNull();
  expect(screen.queryByRole("button", { name: "新增词条" })).toBeNull();
});
