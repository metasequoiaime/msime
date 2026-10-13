// @vitest-environment jsdom
import { settingsFormReady } from "../support/settings-form";
import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import {
  SettingsPage,
  UNBATCHED_DICTIONARY_FILE_BYTES,
  describeImportResult,
  type Snapshot,
} from "@msime/ui";
import {
  bytesToBase64,
  isScelBytes,
  isScelFile,
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
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  if (!kind) return;
  const select = screen.getByLabelText("本地词库类型") as HTMLSelectElement;
  fireEvent.change(select, { target: { value: kind } });
  // 换类型会重新读词条；读完之前页面不接新的操作。
  await waitFor(() => expect(select.disabled).toBe(false));
}

function chooseFile(file: File) {
  const input = document.querySelector('input[type="file"]') as HTMLInputElement;
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
    expect(dictionary.scelText).toHaveBeenCalledWith(Buffer.from(scelBytes).toString("base64"));
    expect(dictionary.import.mock.calls[0].slice(0, 3)).toEqual([
      "pinyin",
      "standard",
      "测试\tce'shi\n实例\tshi'li\n越界\t\n",
    ]);
    const notice = await screen.findByRole("status");
    expect(notice.textContent).toBe(
      "已读取搜狗细胞词库「化学」的 3 个词条。全拼导入完成，共 2 条。跳过 1 行，首先出现在第 3 行。",
    );
    // 选中的格式跟着文件换成细胞词库，它只能导入，所以导出按钮不可用。
    expect((screen.getByLabelText("本地词库文件格式") as HTMLSelectElement).value).toBe("scel");
    expect(
      (screen.getByRole("button", { name: "导出当前类型" }) as HTMLButtonElement).disabled,
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

  test("the format and the .scel filter appear only for pinyin on a host that converts", async () => {
    const dictionary = scelDictionary();
    await openDictionaryPage(dictionary);
    const optionValues = () =>
      Array.from(
        (screen.getByLabelText("本地词库文件格式") as HTMLSelectElement).options,
        (option) => option.value,
      );
    expect(optionValues()).not.toContain("scel");
    fireEvent.change(screen.getByLabelText("本地词库类型"), { target: { value: "pinyin" } });
    expect(optionValues()).toContain("scel");
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    expect(input.accept).toContain(".scel");
    cleanup();

    const { scelText: _scelText, ...withoutScel } = scelDictionary();
    await openDictionaryPage(withoutScel, "pinyin");
    expect(optionValues()).not.toContain("scel");
    expect((document.querySelector('input[type="file"]') as HTMLInputElement).accept).not.toContain(
      ".scel",
    );
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
  expect(bytesToBase64(bytes)).toBe(Buffer.from(bytes).toString("base64"));
  expect(bytesToBase64(new Uint8Array())).toBe("");
});

test("a cell dictionary is recognised by its first four bytes", async () => {
  expect(isScelBytes(scelBytes)).toBe(true);
  expect(isScelBytes(new Uint8Array([0x40, 0x15, 0x00]))).toBe(false);
  expect(isScelBytes(new TextEncoder().encode("你好\tni'hao"))).toBe(false);
  expect(await isScelFile(new File([scelBytes], "a.bin"))).toBe(true);
});
