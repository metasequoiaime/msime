// @vitest-environment jsdom
import { selectDictionaryTab, settingsFormReady } from "../support/settings-form";
import { testHost } from "../support/host";
import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import {
  SettingsPage,
  UNBATCHED_DICTIONARY_FILE_BYTES,
  describeImportResult,
  type Snapshot,
} from "@msime/ui";
import {
  UNBATCHED_IMPORT_TEXT_BYTES,
  bytesToBase64,
  isScelBytes,
  isScelFile,
  leadingImportRows,
} from "../../../../packages/ui/src/dictionary/dictionary-file";

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

describe("describeImportResult", () => {
  test("a clean import states the count only", () => {
    const message = describeImportResult("快捷短语", { applied: 12 });
    expect(message).toContain("12");
    expect(message).not.toContain("跳过");
    expect(message).not.toContain("过长");
  });

  test("skipped rows are counted and the first line numbers named", () => {
    const message = describeImportResult("全拼", {
      applied: 8,
      failed: 3,
      first_failures: [
        { line: 2, issue: "column_count" },
        { line: 9, issue: "key_alphabet" },
      ],
    });
    expect(message).toContain("8");
    expect(message).toContain("跳过 3 行");
    expect(message).toContain("2、9");
  });

  test("a truncated import says so", () => {
    expect(describeImportResult("五笔", { applied: 1000, truncated: true })).toContain("过长");
  });

  test("a file read in the other column order says so", () => {
    const message = describeImportResult("五笔", { applied: 4, swapped: true });
    expect(message).toContain("相反");
    // It is not a failure: the rows all landed.
    expect(message).not.toContain("跳过");
    expect(describeImportResult("五笔", { applied: 4 })).not.toContain("相反");
  });

  test("a host without the report degrades to the count", () => {
    // Older hosts return {applied} only; the message must not invent failures.
    const message = describeImportResult("英文", { applied: 5 });
    expect(message).toContain("5");
    expect(message).not.toContain("跳过");
  });
});

async function openDictionaryPage(dictionary: Record<string, unknown>, kind?: string) {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), dictionary: dictionary as never }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: /^(词库|词库管理)$/ }));
  if (kind) chooseKind(kind);
  // 换类型会重新读词条；读完之前页面不接新的操作。
  const select = screen.getByLabelText<HTMLSelectElement>("本地词库类型");
  await waitFor(() => expect(select.disabled).toBe(false));
}

/** 词库类型在桌面默认的「查词库」标签里选。 */
function chooseKind(kind: string) {
  selectDictionaryTab("查词库");
  fireEvent.change(screen.getByRole("combobox", { name: "本地词库类型" }), {
    target: { value: kind },
  });
}

/** 桌面的导入导出在「管理词库」标签的「导入与导出」组，切过去才看得到。 */
function transferGroup(): HTMLElement {
  selectDictionaryTab("管理词库");
  return screen.getByRole("region", { name: "导入与导出" });
}

/** 像用户一样到「管理词库」标签里点「导入」选文件；导入结果要在这个标签上看得到。 */
function chooseFile(file: File) {
  const input = within(transferGroup()).getByLabelText<HTMLInputElement>("导入");
  Object.defineProperty(input, "files", { value: [file], configurable: true });
  fireEvent.change(input);
}

async function importFile(
  dictionary: Record<string, unknown>,
  file = new File(["你好\tni'hao\n"], "dict.txt", { type: "text/plain" }),
  kind?: string,
) {
  await openDictionaryPage(dictionary, kind);
  chooseFile(file);
}

/** 细胞词库的文件头加几个字节：页面只看前 4 字节认出它，内容由（模拟的）宿主转换。 */
const scelBytes = new Uint8Array([
  0x40, 0x15, 0x00, 0x00, 0x44, 0x43, 0x53, 0x01, 0x01, 0, 0, 0, 7,
]);

function scelDictionary(scelText = vi.fn()) {
  return {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    export: vi.fn().mockResolvedValue({ text: "", has_more: false }),
    import: vi.fn().mockResolvedValue({
      applied: 2,
      failed: 1,
      first_failures: [{ line: 3, issue: "empty_key" }],
    }),
    scelText: scelText.mockResolvedValue({
      name: "化学",
      category: "自然科学",
      entries: 3,
      text: "测试\tce'shi\n实例\tshi'li\n越界\t\n",
    }),
  };
}

test("an import that skipped rows reports them instead of looking clean", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockResolvedValue({
      applied: 2,
      failed: 1,
      first_failures: [{ line: 2, issue: "column_count" }],
    }),
  };
  await importFile(dictionary);
  await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
  const notice = await screen.findByRole("status");
  expect(notice.textContent).toContain("跳过 1 行");
  expect(notice.textContent).toContain("第 2 行");
});

// 手机词库页没有顶部标签，导入导出就在本地词库管理组下面，结果仍画在那一组里，布局不跟着桌面改。
test("on a phone the import result stays in the local dictionary group", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockResolvedValue({ applied: 3 }),
  };
  render(
    <SettingsPage
      initialPage="dictionary"
      client={{
        load: async () => snapshot,
        save: vi.fn(),
        host: testHost({ platform: "ios" }),
        dictionary: dictionary as never,
      }}
    />,
  );
  await settingsFormReady();
  expect(screen.queryByRole("tablist", { name: "词库设置分类" })).toBeNull();
  const input = within(
    screen.getByRole("region", { name: "导入与导出" }),
  ).getByLabelText<HTMLInputElement>("导入");
  const file = new File(["你好\tni'hao\n"], "dict.txt", { type: "text/plain" });
  Object.defineProperty(input, "files", { value: [file], configurable: true });
  fireEvent.change(input);
  const local = screen.getByRole("region", { name: "本地词库管理" });
  expect((await within(local).findByRole("status")).textContent).toContain("3");
});

test("a failed import surfaces the host's reason rather than a generic hint", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockRejectedValue(new Error("dictionary import is too large")),
  };
  await importFile(dictionary);
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("dictionary import is too large");
});

test("an import the host finds too large names the size limits", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockRejectedValue({ code: "dictionary_too_large" }),
  };
  await importFile(dictionary);
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("导入失败：词库文件过大");
  expect(alert.textContent).toContain("32 MB");
  expect(alert.textContent).toContain("拆分");
});

test("a file over 32 MB is refused before anything is sent", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockResolvedValue({ applied: 1 }),
  };
  const large = new File(["a".repeat(33 * 1024 * 1024)], "large.txt", { type: "text/plain" });
  await importFile(dictionary, large);
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("文件不能超过 32 MB");
  expect(dictionary.import).not.toHaveBeenCalled();
});

test("a 2 MB file, over the page's old 1 MB bound, goes to the desktop bridge whole", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockResolvedValue({ applied: 100_000 }),
  };
  const text = Array.from({ length: 100_000 }, (_, index) => `词条${index}\tcitiao\t100\n`).join(
    "",
  );
  const file = new File([text], "large.txt", { type: "text/plain" });
  expect(file.size).toBeGreaterThan(2 * 1024 * 1024);
  await importFile(dictionary, file);
  await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
  expect(dictionary.import.mock.calls[0][2]).toBe(text);
  expect((await screen.findByRole("status")).textContent).toContain("100000");
});

test("a host that sends the file in one request keeps its own bound", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockResolvedValue({ applied: 1 }),
    maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES,
  };
  const large = new File(["a".repeat(2 * 1024 * 1024)], "large.txt", { type: "text/plain" });
  await importFile(dictionary, large);
  const alert = await screen.findByRole("alert");
  expect(alert.textContent).toContain("文件不能超过 1 MB");
  expect(dictionary.import).not.toHaveBeenCalled();
});

test("a file between 64 KiB and 1 MB goes to the host whole", async () => {
  const dictionary = {
    list: vi.fn().mockResolvedValue({ entries: [], has_more: false }),
    edit: vi.fn(),
    import: vi.fn().mockResolvedValue({ applied: 5000 }),
  };
  const text = Array.from({ length: 5000 }, (_, index) => `词${index}\tci\n`).join("");
  await importFile(dictionary, new File([text], "large.txt", { type: "text/plain" }));
  await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
  expect(dictionary.import.mock.calls[0][2]).toBe(text);
  expect((await screen.findByRole("status")).textContent).toContain("5000");
});

test("an engine rejection is explained differently from a malformed line", () => {
  // These parse cleanly but the engine refuses them, so "check the text
  // format" would send the user looking in the wrong place.
  const message = describeImportResult("全拼", {
    applied: 40,
    failed: 2,
    first_failures: [
      { line: 7, issue: "rejected" },
      { line: 12, issue: "rejected" },
    ],
  });
  expect(message).toContain("40");
  expect(message).toContain("跳过 2 行");
  expect(message).toContain("7、12");
  expect(message).toContain("音节");

  // A purely malformed file keeps the original wording.
  const malformed = describeImportResult("全拼", {
    applied: 3,
    failed: 1,
    first_failures: [{ line: 2, issue: "column_count" }],
  });
  expect(malformed).not.toContain("音节");
});

describe("Sogou cell dictionaries", () => {
  test("a .scel file is converted by the host and imported as standard pinyin rows", async () => {
    const dictionary = scelDictionary();
    await importFile(dictionary, new File([scelBytes], "化学.scel"), "pinyin");
    await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
    expect(dictionary.scelText).toHaveBeenCalledWith("QBUAAERDUwEBAAAABw==");
    expect(dictionary.import.mock.calls[0].slice(0, 3)).toEqual([
      "pinyin",
      "standard",
      "测试\tce'shi\n实例\tshi'li\n越界\t\n",
    ]);
    const notice = await screen.findByRole("status");
    expect(notice.textContent).toBe(
      "已读取搜狗细胞词库「化学」的 3 个词条。拼音导入完成，共 2 条。跳过 1 行，首先出现在第 3 行。搜狗细胞词库的整理成果属于原作者，导入的词只供你在本机使用，不能发布到社区。",
    );
    // 选中的格式跟着文件换成细胞词库，它只能导入，所以导出按钮不可用。
    const transfer = transferGroup();
    expect(
      within(transfer).getByRole<HTMLSelectElement>("combobox", { name: "本地词库文件格式" }).value,
    ).toBe("scel");
    expect(
      within(transfer).getByRole<HTMLButtonElement>("button", { name: "导出当前类型" }).disabled,
    ).toBe(true);
  });

  test("a cell dictionary renamed to .txt is recognised by its header", async () => {
    const dictionary = scelDictionary();
    await importFile(dictionary, new File([scelBytes], "化学.txt"), "pinyin");
    await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
    expect(dictionary.scelText).toHaveBeenCalled();
    expect(dictionary.import.mock.calls[0][1]).toBe("standard");
  });

  test("only a pinyin dictionary takes a cell dictionary", async () => {
    const dictionary = scelDictionary();
    await importFile(dictionary, new File([scelBytes], "化学.scel"));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("搜狗细胞词库只能导入拼音词库，请先把「词库」切换到拼音。");
    expect(dictionary.scelText).not.toHaveBeenCalled();
    expect(dictionary.import).not.toHaveBeenCalled();
  });

  test("the host's refusal reads as a sentence", async () => {
    const dictionary = scelDictionary();
    dictionary.scelText.mockReset();
    dictionary.scelText.mockRejectedValue({ code: "dictionary_scel_malformed" });
    await importFile(dictionary, new File([scelBytes], "化学.scel"), "pinyin");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("细胞词库文件不完整或已损坏，请重新下载后再导入。");
    expect(dictionary.import).not.toHaveBeenCalled();
  });

  test("a cell dictionary over the host's bound is refused before it is read", async () => {
    const dictionary = { ...scelDictionary(), maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES };
    const large = new Uint8Array(2 * 1024 * 1024);
    large.set(scelBytes);
    await importFile(dictionary, new File([large], "化学.scel"), "pinyin");
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("细胞词库不能超过 1 MB。");
    expect(dictionary.scelText).not.toHaveBeenCalled();
  });

  test("a host that imports in one request gets only the rows its parser reads", async () => {
    const dictionary = { ...scelDictionary(), maxImportFileBytes: UNBATCHED_DICTIONARY_FILE_BYTES };
    // 一个约 100 KB 的转出文本：超过不分批宿主一次收的 64 KiB，整份送过去会被整份拒绝。
    const text = Array.from({ length: 5000 }, (_, index) => `词条${index}\tci'tiao\n`).join("");
    dictionary.scelText.mockResolvedValue({ name: "化学", category: "", entries: 5000, text });
    // 报告里没有 truncated 也要说文件过长：先到 64 KiB 时解析器看不到后面还有行。
    dictionary.import.mockResolvedValue({ applied: 1000 });
    await importFile(dictionary, new File([scelBytes], "化学.scel"), "pinyin");
    await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
    const sent = dictionary.import.mock.calls[0][2] as string;
    expect(sent.split("\n").filter(Boolean)).toHaveLength(1001);
    expect(text.startsWith(sent)).toBe(true);
    expect((await screen.findByRole("status")).textContent).toContain("文件过长");
  });

  test("the desktop bridge gets the whole converted text", async () => {
    const dictionary = scelDictionary();
    const text = Array.from({ length: 5000 }, (_, index) => `词条${index}\tci'tiao\n`).join("");
    dictionary.scelText.mockResolvedValue({ name: "化学", category: "", entries: 5000, text });
    await importFile(dictionary, new File([scelBytes], "化学.scel"), "pinyin");
    await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
    expect(dictionary.import.mock.calls[0][2]).toBe(text);
  });

  test("the format and the .scel filter appear only for pinyin on a host that converts", async () => {
    const dictionary = scelDictionary();
    await openDictionaryPage(dictionary);
    const optionValues = () =>
      Array.from(
        within(transferGroup()).getByRole<HTMLSelectElement>("combobox", {
          name: "本地词库文件格式",
        }).options,
        (option) => option.value,
      );
    const importAccept = () =>
      within(transferGroup()).getByLabelText<HTMLInputElement>("导入").accept;
    expect(optionValues()).not.toContain("scel");
    chooseKind("pinyin");
    expect(optionValues()).toContain("scel");
    expect(importAccept()).toContain(".scel");
    cleanup();

    const { scelText: _scelText, ...withoutScel } = scelDictionary();
    await openDictionaryPage(withoutScel, "pinyin");
    expect(optionValues()).not.toContain("scel");
    expect(importAccept()).not.toContain(".scel");
  });

  test("a .yaml file is read as Rime on the first pick", async () => {
    const dictionary = scelDictionary();
    await importFile(
      dictionary,
      new File(["你好\tni hao\t1\n"], "luna.dict.yaml", { type: "text/plain" }),
      "pinyin",
    );
    await waitFor(() => expect(dictionary.import).toHaveBeenCalled());
    expect(dictionary.import.mock.calls[0][1]).toBe("rime");
    expect(dictionary.scelText).not.toHaveBeenCalled();
  });
});

test("bytes are encoded to base64 in chunks without overflowing the stack", () => {
  const bytes = new Uint8Array(3 * 1024 * 1024 + 2);
  for (let index = 0; index < bytes.length; index++) bytes[index] = (index * 31 + 7) & 0xff;
  // 对照：整段字节一次拼成二进制串再交给 `btoa`，不分块。
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  expect(bytesToBase64(bytes)).toBe(btoa(binary));
  expect(bytesToBase64(new Uint8Array())).toBe("");
});

test("leading rows stop at 1001 rows or the parser's 64 KiB, whichever comes first", () => {
  const short = Array.from({ length: 1500 }, (_, index) => `词${index}\tci\n`).join("");
  const rows = leadingImportRows(short);
  expect(rows.text.split("\n").filter(Boolean)).toHaveLength(1001);
  expect(rows.cut).toBe(true);
  expect(short.startsWith(rows.text)).toBe(true);

  // 词长的词库（诗词、俗语）不到 1001 行就到了 64 KiB：只送放得下的整行，并且标出有行没送。
  const row = `${"长".repeat(20)}\t${Array(20).fill("chang").join("'")}\n`;
  const long = leadingImportRows(row.repeat(1500));
  expect(new TextEncoder().encode(long.text).length).toBeLessThanOrEqual(
    UNBATCHED_IMPORT_TEXT_BYTES,
  );
  expect(long.text.length % row.length).toBe(0);
  expect(long.cut).toBe(true);

  const small = "测试\tce'shi\n实例\tshi'li";
  expect(leadingImportRows(small)).toEqual({ text: small, cut: false });
});

test("a cell dictionary is recognised by its first four bytes", async () => {
  expect(isScelBytes(scelBytes)).toBe(true);
  expect(isScelBytes(new Uint8Array([0x40, 0x15, 0x00]))).toBe(false);
  expect(isScelBytes(new TextEncoder().encode("你好\tni'hao"))).toBe(false);
  expect(await isScelFile(new File([scelBytes], "a.bin"))).toBe(true);
});
