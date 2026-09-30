// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SettingsPage, type DictionaryEntry, type Snapshot } from "@msime/ui";
import { answerConfirm } from "../support/confirm";
// Not re-exported from the package root; take it from the module that owns it.
import {
  DICTIONARY_PAGE_SIZE,
  parsePersonalDictionaryImport,
} from "../../../../packages/ui/src/dictionary/dictionary-file";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const snapshot: Snapshot = {
  format_version: 1,
  revision: 3,
  preferences: {
    scheme: "quanpin",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 5,
    learning: true,
    chinese_punctuation: true,
  },
};

function entries(count: number): DictionaryEntry[] {
  return Array.from({ length: count }, (_, index) => ({
    kind: "quick_phrase" as const,
    key: `k${index}`,
    value: `短语${index}`,
    weight: 100,
  }));
}

/** A dictionary client whose pages always fill, so 下一页 stays enabled. */
function dictionaryClient(overrides: Record<string, unknown> = {}) {
  return {
    list: vi
      .fn()
      .mockImplementation(async () => ({ entries: entries(DICTIONARY_PAGE_SIZE), has_more: true })),
    edit: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

async function openDictionary(dictionary: ReturnType<typeof dictionaryClient>) {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), dictionary: dictionary as never }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  // The page lists on demand rather than on open.
  fireEvent.click(await screen.findByRole("button", { name: "查询" }));
  // Wait for the rows themselves; the call resolving is not the same as a render.
  await screen.findAllByRole("button", { name: "删除" });
}

test("deleting an entry asks first and does nothing when declined", async () => {
  const dictionary = dictionaryClient();
  await openDictionary(dictionary);
  fireEvent.click(screen.getAllByRole("button", { name: "删除" })[0]);
  await answerConfirm("cancel");
  // Declining must not reach the host at all.
  expect(dictionary.edit).not.toHaveBeenCalled();
});

test("a confirmed delete reloads the page the user was reading", async () => {
  const dictionary = dictionaryClient();
  await openDictionary(dictionary);
  // Move to the second page before deleting.
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  await waitFor(() => expect(dictionary.list).toHaveBeenCalledTimes(2));
  expect(dictionary.list.mock.calls[1][0]).toBe(DICTIONARY_PAGE_SIZE);

  fireEvent.click(screen.getAllByRole("button", { name: "删除" })[0]);
  await answerConfirm("confirm");
  await waitFor(() => expect(dictionary.edit).toHaveBeenCalled());
  await waitFor(() => expect(dictionary.list).toHaveBeenCalledTimes(3));
  // Previously this reloaded at offset 0 and threw the reader back to page 1.
  expect(dictionary.list.mock.calls[2][0]).toBe(DICTIONARY_PAGE_SIZE);
});

test("deleting the only row on a later page steps back instead of showing nothing", async () => {
  const dictionary = dictionaryClient({
    list: vi
      .fn()
      .mockImplementation(async (offset: number) =>
        offset === 0
          ? { entries: entries(DICTIONARY_PAGE_SIZE), has_more: true }
          : { entries: entries(1), has_more: false },
      ),
  });
  await openDictionary(dictionary);
  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  await waitFor(() => expect(dictionary.list).toHaveBeenCalledTimes(2));

  fireEvent.click(screen.getAllByRole("button", { name: "删除" })[0]);
  await answerConfirm("confirm");
  await waitFor(() => expect(dictionary.list).toHaveBeenCalledTimes(3));
  expect(dictionary.list.mock.calls[2][0]).toBe(0);
});

test("a bundled row is badged and its edit only changes the weight", async () => {
  const bundled: DictionaryEntry = {
    kind: "quick_phrase",
    key: "dh",
    value: "电话",
    weight: 500,
    source: "bundled",
  };
  const user: DictionaryEntry = { ...bundled, key: "wd", value: "我的", source: "user" };
  const dictionary = dictionaryClient({
    list: vi.fn().mockResolvedValue({ entries: [bundled, user], has_more: false }),
  });
  await openDictionary(dictionary);
  expect(screen.getAllByText("内置")).toHaveLength(1);
  // A user row keeps the full editor; the bundled one offers only a weight change.
  expect(screen.getByRole("button", { name: "编辑" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "调权重" }));
  const code = screen.getByDisplayValue("dh") as HTMLInputElement;
  const phrase = screen.getByDisplayValue("电话") as HTMLInputElement;
  expect(code.readOnly && phrase.readOnly).toBe(true);
  fireEvent.change(screen.getByDisplayValue("500"), { target: { value: "1" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(dictionary.edit).toHaveBeenCalled());
  const [previous, replacement] = dictionary.edit.mock.calls[0];
  expect(previous).toEqual(bundled);
  expect(replacement).toEqual({ ...bundled, weight: 1 });
});

test("Android personal dictionary JSON import previews and queues only after confirmation", async () => {
  const importPersonal = vi.fn().mockResolvedValue({ queued: true, pending_count: 2 });
  const dictionary = dictionaryClient({ importPersonal });
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), dictionary: dictionary as never }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));

  const file = new File(
    [
      JSON.stringify({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [
          { kind: "pinyin", key: "ni hao", value: "你好", weight: 100000 },
          { kind: "quickPhrase", key: "hello", value: "你好！", weight: 3 },
        ],
      }),
    ],
    "personal.json",
    { type: "application/json" },
  );
  fireEvent.change(screen.getByLabelText("选择个人词库 JSON 文件"), { target: { files: [file] } });
  expect(importPersonal).not.toHaveBeenCalled();
  expect(await screen.findByText(/已校验 2 条（/)).not.toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "确认导入" }));
  await waitFor(() =>
    expect(importPersonal).toHaveBeenCalledWith(
      expect.stringContaining("msime-personal-dictionary"),
      expect.stringMatching(/^ui-personal-import-/),
    ),
  );
  expect(await screen.findByText(/已加入本机同步队列/)).not.toBeNull();
});

test("personal dictionary import ignores a response from a replaced dictionary client", async () => {
  let resolveImport!: (value: { queued: boolean; pending_count: number }) => void;
  const oldImport = vi.fn(
    () =>
      new Promise<{ queued: boolean; pending_count: number }>(
        (resolve) => (resolveImport = resolve),
      ),
  );
  const oldDictionary = dictionaryClient({ importPersonal: oldImport });
  const view = render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), dictionary: oldDictionary as never }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  const file = new File(
    [
      JSON.stringify({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [{ kind: "pinyin", key: "ni", value: "你", weight: 1 }],
      }),
    ],
    "stale.json",
    { type: "application/json" },
  );
  fireEvent.change(screen.getByLabelText("选择个人词库 JSON 文件"), { target: { files: [file] } });
  await screen.findByText(/已校验 1 条/);
  fireEvent.click(screen.getByRole("button", { name: "确认导入" }));
  await waitFor(() => expect(oldImport).toHaveBeenCalled());
  const nextDictionary = dictionaryClient({
    importPersonal: vi.fn().mockResolvedValue({ queued: true, pending_count: 0 }),
  });
  view.rerender(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), dictionary: nextDictionary as never }}
    />,
  );
  resolveImport({ queued: true, pending_count: 1 });
  await Promise.resolve();
  expect(screen.queryByText(/已加入本机同步队列，共 1 条/)).toBeNull();
});

test("personal dictionary JSON validation normalizes before keeping malformed and duplicate entries out", () => {
  expect(() =>
    parsePersonalDictionaryImport(
      JSON.stringify({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [
          { kind: "pinyin", key: "ni hao", value: "你好", weight: 1 },
          { kind: "pinyin", key: "ni hao", value: "你好", weight: 2 },
        ],
      }),
    ),
  ).toThrow("重复");
  expect(() =>
    parsePersonalDictionaryImport(
      JSON.stringify({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [{ kind: "quickPhrase", key: "BAD;CODE", value: "坏", weight: 1 }],
      }),
    ),
  ).toThrow("输入引擎规则");
  expect(
    parsePersonalDictionaryImport(
      JSON.stringify({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [{ kind: "pinyin", key: "NI HAO", value: "拟好", weight: 1 }],
      }),
    )[0].key,
  ).toBe("ni'hao");
});
