// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import type { DictionaryClient } from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import { exportPersonalPinyinDictionary } from "../../../../packages/ui/src/dictionary/dictionary-export-action";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("缺少导出能力时静默返回，不调用保存", async () => {
  const save = vi.fn();
  await expect(exportPersonalPinyinDictionary({}, save)).resolves.toBe("");
  expect(save).not.toHaveBeenCalled();
});

test("逐页读取并保留方法接收者，过滤单字后交给宿主保存", async () => {
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
        ? { text: "甲\tjia\t1\n合成\thecheng\t2\n", has_more: true }
        : { text: "测试\tceshi\t3\n", has_more: false };
    }),
  };
  const save = vi.fn(async () => "/synthetic/pinyin.txt");
  await expect(exportPersonalPinyinDictionary(dictionary, save)).resolves.toBe("已导出 2 条");
  expect(dictionary.export.mock.calls).toEqual([
    ["pinyin", "standard", 0, 1000],
    ["pinyin", "standard", 2, 1000],
  ]);
  expect(save).toHaveBeenCalledExactlyOnceWith(
    "水杉IME-拼音用户词库.txt",
    "\ufeff合成\thecheng\t2\n测试\tceshi\t3\n",
  );
});

test.each(["", "甲\tjia\t1\n"])("过滤后没有词条时不保存：%j", async (text) => {
  const save = vi.fn();
  await expect(
    exportPersonalPinyinDictionary({ export: async () => ({ text, has_more: false }) }, save),
  ).resolves.toBe("还没有自己加过的拼音词条。");
  expect(save).not.toHaveBeenCalled();
});

test.each([
  [null, ""],
  ["", "已导出 1 条"],
])("保存返回 %j 时保留取消与成功语义", async (path, notice) => {
  const save = vi.fn(async () => path);
  await expect(
    exportPersonalPinyinDictionary(
      { export: async () => ({ text: "合成\thecheng\t2\n", has_more: false }) },
      save,
    ),
  ).resolves.toBe(notice);
});

test.each([
  [new Error("合成错误"), "合成错误"],
  [new Error("dictionary_export_limit"), "词库导出失败，请稍后重试。"],
  [new Error(), "词库导出失败，请稍后重试。"],
  ["synthetic failure", "词库导出失败，请稍后重试。"],
])("读取与保存失败使用相同的提示规则：%j", async (error, notice) => {
  const save = vi.fn(async () => {
    throw error;
  });
  await expect(
    exportPersonalPinyinDictionary(
      {
        export: async () => {
          throw error;
        },
      },
      save,
    ),
  ).resolves.toBe(notice);
  expect(save).not.toHaveBeenCalled();
  await expect(
    exportPersonalPinyinDictionary(
      { export: async () => ({ text: "合成\thecheng\t2\n", has_more: false }) },
      save,
    ),
  ).resolves.toBe(notice);
});

test("没有宿主保存能力时通过公共下载方法交付 UTF-8 文件", async () => {
  const createObjectURL = vi.fn<(blob: Blob) => string>(() => "blob:synthetic-export");
  vi.stubGlobal("URL", { createObjectURL, revokeObjectURL: vi.fn() });
  vi.spyOn(window, "setTimeout").mockReturnValue(0);
  const downloads: string[] = [];
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
    this: HTMLAnchorElement,
  ) {
    downloads.push(this.download);
    expect(this.href).toBe("blob:synthetic-export");
  });
  await expect(
    exportPersonalPinyinDictionary({
      export: async () => ({ text: "合成\thecheng\t2\n", has_more: false }),
    }),
  ).resolves.toBe("已导出 1 条");
  expect(downloads).toEqual(["水杉IME-拼音用户词库.txt"]);
  const blob = createObjectURL.mock.calls[0]![0] as Blob;
  expect(blob.type).toBe("text/plain;charset=utf-8");
  const bytes = await new Promise<ArrayBuffer>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.onerror = () => reject(reader.error);
    reader.readAsArrayBuffer(blob);
  });
  expect(new TextDecoder("utf-8", { ignoreBOM: true }).decode(bytes)).toBe(
    "\ufeff合成\thecheng\t2\n",
  );
});

test("浏览器下载失败也返回通用失败提示", async () => {
  vi.stubGlobal("URL", {
    createObjectURL: () => {
      throw "synthetic failure";
    },
  });
  await expect(
    exportPersonalPinyinDictionary({
      export: async () => ({ text: "合成\thecheng\t2\n", has_more: false }),
    }),
  ).resolves.toBe("词库导出失败，请稍后重试。");
});
