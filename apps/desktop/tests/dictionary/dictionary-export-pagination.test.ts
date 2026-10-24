import { expect, test, vi } from "vitest";
import type { DictionaryClient } from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import { loadDictionaryExportText } from "../../../../packages/ui/src/dictionary/dictionary-export";

test("按返回行数分页并原样拼接文本，保留方法接收者", async () => {
  const dictionary = {
    marker: "synthetic",
    export: vi.fn<NonNullable<DictionaryClient["export"]>>(async function (
      this: { marker: string },
      _kind,
      _format,
      offset,
    ) {
      expect(this.marker).toBe("synthetic");
      return offset === 0
        ? { text: "ab\t合成\t1\n\ncd\t测试\t2\n", has_more: true }
        : { text: "ef\t末页\t3\n", has_more: false };
    }),
  };
  await expect(loadDictionaryExportText(dictionary, "wubi98", "windows")).resolves.toBe(
    "ab\t合成\t1\n\ncd\t测试\t2\nef\t末页\t3\n",
  );
  expect(dictionary.export.mock.calls).toEqual([
    ["wubi98", "windows", 0, 1000],
    ["wubi98", "windows", 3, 1000],
  ]);
});

test.each(["", "\n\n"])("空文本 %j 保留既有计数及停止规则", async (text) => {
  const exportPage = vi
    .fn()
    .mockResolvedValueOnce({ text, has_more: true })
    .mockResolvedValueOnce({ text: "", has_more: true });
  await expect(
    loadDictionaryExportText({ export: exportPage }, "pinyin", "standard"),
  ).resolves.toBe(text);
  expect(exportPage).toHaveBeenCalledTimes(text ? 2 : 1);
  if (text) expect(exportPage).toHaveBeenLastCalledWith("pinyin", "standard", 1, 1000);
});

test("缺少导出能力时返回空文本", async () => {
  await expect(loadDictionaryExportText({}, "english", "standard")).resolves.toBe("");
});

test("累计 UTF-8 字节恰好达到上限时仍可导出", async () => {
  const exportPage = vi
    .fn()
    .mockResolvedValueOnce({ text: "词\n", has_more: true })
    .mockResolvedValueOnce({ text: "句\n", has_more: false });
  await expect(
    loadDictionaryExportText({ export: exportPage }, "pinyin", "standard", 8),
  ).resolves.toBe("词\n句\n");
});

test("累计 UTF-8 字节超限后停止请求", async () => {
  const exportPage = vi
    .fn()
    .mockResolvedValueOnce({ text: "词\n", has_more: true })
    .mockResolvedValueOnce({ text: "句\n", has_more: true });
  await expect(
    loadDictionaryExportText({ export: exportPage }, "pinyin", "standard", 7),
  ).rejects.toThrow("dictionary_export_limit");
  expect(exportPage).toHaveBeenCalledTimes(2);
});

test("未指定原始文本上限时保留全部文本", async () => {
  const exportPage = vi.fn().mockResolvedValue({ text: "词\n句\n", has_more: false });
  await expect(
    loadDictionaryExportText({ export: exportPage }, "pinyin", "standard"),
  ).resolves.toBe("词\n句\n");
});

test("宿主错误原样交还调用方", async () => {
  const error = new Error("synthetic export failure");
  const exportPage = vi.fn().mockRejectedValue(error);
  await expect(
    loadDictionaryExportText({ export: exportPage }, "english", "standard"),
  ).rejects.toBe(error);
});

test("偏移等于百万时仍读取，超过后按既有规则返回", async () => {
  const first = "x\n".repeat(1_000_000);
  const exportPage = vi
    .fn()
    .mockResolvedValueOnce({ text: first, has_more: true })
    .mockResolvedValueOnce({ text: "last\n", has_more: true });
  await expect(
    loadDictionaryExportText({ export: exportPage }, "english", "standard"),
  ).resolves.toBe(first + "last\n");
  expect(exportPage.mock.calls).toEqual([
    ["english", "standard", 0, 1000],
    ["english", "standard", 1_000_000, 1000],
  ]);
});
