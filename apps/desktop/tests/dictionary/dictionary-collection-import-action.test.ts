import { expect, test, vi } from "vitest";
import type { DictionaryCollectionsView } from "../../../../packages/ui/src/dictionary/dictionary-collections";
import {
  importDictionaryCollectionFile,
  type DictionaryCollectionImportOptions,
} from "../../../../packages/ui/src/dictionary/dictionary-collection-import-action";

const imported: DictionaryCollectionsView = {
  collections: [],
  formats: ["txt", "rime", "hans", "scel"],
  import: { imported: 2, duplicates: 1, failed: 3, truncated: true, swapped: false },
};

function options(file = new File(["合成\thecheng\t10\n"], "合成词库.txt")) {
  const collections = {
    importFile: vi.fn(async () => imported),
    importBytes: vi.fn(async () => imported),
  };
  const run = vi.fn<DictionaryCollectionImportOptions["run"]>((action) => action());
  return { collections, source: { format: "txt" }, file, isCurrent: () => true, run };
}

function pendingRead() {
  let resolve!: (value: ArrayBuffer) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<ArrayBuffer>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test("独立导入文本，名称、读取内容和完整摘要交还调用方", async () => {
  const input = options();
  await expect(importDictionaryCollectionFile(input)).resolves.toBe(
    "已导入「合成词库」，2 条，跳过重复 1 条，3 行无法识别，词库已满",
  );
  expect(input.run).toHaveBeenCalledOnce();
  expect(input.collections.importFile).toHaveBeenCalledExactlyOnceWith(
    "合成词库",
    "txt",
    "合成\thecheng\t10\n",
  );
  expect(input.collections.importBytes).not.toHaveBeenCalled();
});

test.each(["YAML", "yml", "dict.yaml"])("文件扩展名 %s 优先选择 Rime 格式", async (suffix) => {
  const input = options(new File(["synthetic"], `合成词库.${suffix}`));
  await importDictionaryCollectionFile(input);
  expect(input.collections.importFile).toHaveBeenCalledWith("合成词库", "rime", "synthetic");
});

test("普通文件保留选择的导入格式并复用 UTF-16 解码", async () => {
  const input = options(
    new File([new Uint8Array([0xff, 0xfe, 0x08, 0x54, 0x10, 0x62])], "合成.txt"),
  );
  input.source.format = "hans";
  await importDictionaryCollectionFile(input);
  expect(input.collections.importFile).toHaveBeenCalledWith("合成", "hans", "合成");
});

test("细胞词库由扩展名识别，按原始字节编码并附带本机使用提示", async () => {
  const input = options(new File([new Uint8Array([0x40, 0x15, 0, 0, 0xff])], "合成.SCEL"));
  const message = await importDictionaryCollectionFile(input);
  expect(input.collections.importBytes).toHaveBeenCalledExactlyOnceWith("合成", "scel", "QBUAAP8=");
  expect(input.collections.importFile).not.toHaveBeenCalled();
  expect(message).toBe(
    "已导入「合成」，2 条，跳过重复 1 条，3 行无法识别，词库已满。搜狗细胞词库的整理成果属于原作者，导入的词只供你在本机使用，不能发布到社区。",
  );
});

test("缺少二进制能力时不读取文件、不进入修改", async () => {
  const input = options(new File(["synthetic"], "合成.scel"));
  const read = vi.spyOn(input.file, "arrayBuffer");
  await expect(
    importDictionaryCollectionFile({
      ...input,
      collections: { importFile: input.collections.importFile },
    }),
  ).resolves.toBe("暂不支持这种文件格式。");
  expect(read).not.toHaveBeenCalled();
  expect(input.run).not.toHaveBeenCalled();
});

test.each([
  ["txt", 16 * 1024 * 1024, "文件不能超过 16 MB，请拆分后分别导入。"],
  ["scel", 8 * 1024 * 1024, "细胞词库不能超过 8 MB。"],
] as const)("%s 超限文件在读取前拒绝", async (format, limit, message) => {
  const input = options(new File([new Uint8Array(limit + 1)], `合成.${format}`));
  const read = vi.spyOn(input.file, "arrayBuffer");
  await expect(importDictionaryCollectionFile(input)).resolves.toBe(message);
  expect(read).not.toHaveBeenCalled();
  expect(input.run).not.toHaveBeenCalled();
});

test.each([new Error("合成读取失败"), { code: "synthetic_read_failure" }])(
  "读取失败保留原提示或回退提示",
  async (error) => {
    const input = options();
    vi.spyOn(input.file, "arrayBuffer").mockRejectedValue(error);
    await expect(importDictionaryCollectionFile(input)).resolves.toBe(
      error instanceof Error ? error.message : "读取文件失败，请重新选择。",
    );
    expect(input.run).not.toHaveBeenCalled();
  },
);

test.each(["txt", "scel"])("读取 %s 期间失效，不向旧宿主提交", async (format) => {
  const input = options(new File(["synthetic"], `合成.${format}`));
  const reading = pendingRead();
  let current = true;
  vi.spyOn(input.file, "arrayBuffer").mockReturnValue(reading.promise);
  const result = importDictionaryCollectionFile({ ...input, isCurrent: () => current });
  current = false;
  reading.resolve(new Uint8Array([0x40, 0x15, 0, 0]).buffer);
  await expect(result).resolves.toBe("");
  expect(input.run).not.toHaveBeenCalled();
});

test("过期的读取失败不返回提示", async () => {
  const input = options();
  const reading = pendingRead();
  let current = true;
  vi.spyOn(input.file, "arrayBuffer").mockReturnValue(reading.promise);
  const result = importDictionaryCollectionFile({ ...input, isCurrent: () => current });
  current = false;
  reading.reject(new Error("合成旧读取失败"));
  await expect(result).resolves.toBe("");
  expect(input.run).not.toHaveBeenCalled();
});

test("调用前已失效，不读取或修改", async () => {
  const input = options();
  const read = vi.spyOn(input.file, "arrayBuffer");
  await expect(importDictionaryCollectionFile({ ...input, isCurrent: () => false })).resolves.toBe(
    "",
  );
  expect(read).not.toHaveBeenCalled();
  expect(input.run).not.toHaveBeenCalled();
});

test("修改未返回有效视图时不生成成功提示", async () => {
  const input = options();
  input.run.mockResolvedValue(undefined);
  await expect(importDictionaryCollectionFile(input)).resolves.toBe("");
  expect(input.collections.importFile).not.toHaveBeenCalled();
});

test("修改错误交给已有 runner，不改写为读取错误", async () => {
  const input = options();
  const error = { code: "collections_limit" };
  input.run.mockRejectedValue(error);
  await expect(importDictionaryCollectionFile(input)).rejects.toBe(error);
});

test("修改返回时操作已失效，不生成成功提示", async () => {
  const input = options();
  let current = true;
  input.run.mockImplementation(async (action) => {
    const result = await action();
    current = false;
    return result;
  });
  await expect(
    importDictionaryCollectionFile({ ...input, isCurrent: () => current }),
  ).resolves.toBe("");
});

test.each(["txt", "scel"])("%s 导入保留客户端方法接收者", async (format) => {
  const input = options(new File(["synthetic"], `合成.${format}`));
  const collections = {
    marker: "synthetic",
    async importFile() {
      expect(this.marker).toBe("synthetic");
      return imported;
    },
    async importBytes() {
      expect(this.marker).toBe("synthetic");
      return imported;
    },
  };
  await expect(importDictionaryCollectionFile({ ...input, collections })).resolves.toContain(
    "已导入「合成」",
  );
});
