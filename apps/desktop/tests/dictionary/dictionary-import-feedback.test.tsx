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

async function importFile(
  dictionary: Record<string, unknown>,
  file = new File(["你好\tni'hao\n"], "dict.txt", { type: "text/plain" }),
) {
  render(
    <SettingsPage
      client={{ load: async () => snapshot, save: vi.fn(), dictionary: dictionary as never }}
    />,
  );
  await settingsFormReady();
  fireEvent.click(screen.getByRole("button", { name: "词库" }));
  const input = document.querySelector('input[type="file"]') as HTMLInputElement;
  Object.defineProperty(input, "files", { value: [file] });
  fireEvent.change(input);
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
