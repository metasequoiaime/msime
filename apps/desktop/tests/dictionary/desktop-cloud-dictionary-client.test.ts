import { afterEach, expect, test, vi } from "vitest";
import type { CloudDictionaryEntry, CloudDictionaryPanelClient, HostPlatform } from "@msime/ui";
import { parsePersonalDictionaryImport } from "../../../../packages/ui/src/dictionary/dictionary-file";
import { createDesktopCloudDictionaryClient } from "../../src/dictionary/desktop-cloud-dictionary-client";

afterEach(() => vi.restoreAllMocks());

function panelClient() {
  return {
    close: vi.fn(async () => {}),
    request: vi.fn<CloudDictionaryPanelClient["request"]>(async () => ({ saved: true })),
  };
}

const entry: CloudDictionaryEntry = {
  id: "synthetic-entry",
  kind: "pinyin",
  code: "he'cheng",
  word: "合成词条",
  weight: 123,
  revision: 8,
};

test.each<{
  platform: HostPlatform | undefined;
  snapshot: boolean;
  native: boolean;
  download: boolean;
}>([
  { platform: "android", snapshot: true, native: false, download: true },
  { platform: "ios", snapshot: true, native: false, download: true },
  { platform: "macos", snapshot: true, native: true, download: false },
  { platform: "windows", snapshot: true, native: false, download: false },
  { platform: "linux", snapshot: false, native: false, download: false },
  { platform: "harmony", snapshot: false, native: false, download: false },
  { platform: undefined, snapshot: false, native: false, download: false },
])("$platform 面板沿用宿主快照与单词条下载能力", ({ platform, snapshot, native, download }) => {
  const base = panelClient();
  const client = createDesktopCloudDictionaryClient(base, platform, {});
  expect(client.snapshot).toBe(snapshot);
  expect(client.snapshotNative).toBe(native);
  expect(typeof client.downloadToLocal === "function").toBe(download);
  expect(client.snapshotAppliesInPanel).toBe(platform === "windows" ? true : undefined);
  expect(typeof client.chooseSnapshotRestore === "function").toBe(platform === "macos");
  expect(base.request).not.toHaveBeenCalled();
  expect(base.close).not.toHaveBeenCalled();
});

test("复制客户端而不改原对象，保留账号请求和窗口动作", async () => {
  const base = Object.freeze({
    ...panelClient(),
    back: vi.fn(async () => {}),
    saveExport: vi.fn(async (_name: string, _text: string) => {}),
    exportNative: true,
  });
  const client = createDesktopCloudDictionaryClient(base, "windows", {});
  expect(client).not.toBe(base);
  expect(base).not.toHaveProperty("snapshot");
  expect(client.close).toBe(base.close);
  expect(client.request).toBe(base.request);
  expect(client.back).toBe(base.back);
  expect(client.saveExport).toBe(base.saveExport);
  expect(client.exportNative).toBe(true);
  await expect(client.request({ operation: "snapshot_status" })).resolves.toEqual({ saved: true });
  expect(base.request).toHaveBeenCalledExactlyOnceWith({ operation: "snapshot_status" });
});

test("macOS 原生文件选择调用当前账号客户端并返回其结果", async () => {
  const base = panelClient();
  const client = createDesktopCloudDictionaryClient(base, "macos", {});
  await expect(client.chooseSnapshotRestore!()).resolves.toEqual({ saved: true });
  expect(base.request).toHaveBeenCalledExactlyOnceWith({ operation: "snapshot_choose_restore" });
});

test.each(["android", "ios"] as const)(
  "%s 单词条下载生成现有个人词库导入格式",
  async (platform) => {
    vi.spyOn(Date, "now").mockReturnValue(123456);
    const imports: string[] = [];
    const requestIds: string[] = [];
    const client = createDesktopCloudDictionaryClient(panelClient(), platform, {
      importPersonal: async (text, requestId) => {
        imports.push(text);
        requestIds.push(requestId);
        return { queued: true, pending_count: 1 };
      },
    });
    for (const [kind, code, word, localKind] of [
      ["pinyin", "he'cheng", "合成词条", "pinyin"],
      ["english", "synthetic", "synthetic", "english"],
      ["quick", "synthetic", "合成\t短语\n第二行", "quickPhrase"],
    ] as const) {
      await expect(
        client.downloadToLocal!({ ...entry, kind, code, word }),
      ).resolves.toBeUndefined();
      const text = imports.at(-1)!;
      expect(JSON.parse(text)).toEqual({
        format: "msime-personal-dictionary",
        version: 1,
        entries: [{ kind: localKind, key: code, value: word, weight: entry.weight }],
      });
      expect(parsePersonalDictionaryImport(text)).toEqual([
        { kind: localKind, key: code, value: word, weight: entry.weight },
      ]);
    }
    expect(imports).toHaveLength(3);
    expect(requestIds).toEqual(Array(3).fill("ui-cloud-download-123456"));
  },
);

test("移动端动作覆盖原下载回调，桌面平台保留原动作", async () => {
  const downloadToLocal = vi.fn(async (_entry: CloudDictionaryEntry) => {});
  const base = { ...panelClient(), downloadToLocal };
  const importPersonal = vi.fn(async (_text: string, _requestId: string) => ({
    queued: true,
    pending_count: 1,
  }));
  const mobile = createDesktopCloudDictionaryClient(base, "android", { importPersonal });
  const desktop = createDesktopCloudDictionaryClient(base, "linux", { importPersonal });
  expect(desktop.downloadToLocal).toBe(downloadToLocal);
  await mobile.downloadToLocal!(entry);
  expect(importPersonal).toHaveBeenCalledOnce();
  expect(downloadToLocal).not.toHaveBeenCalled();
  expect(base.downloadToLocal).toBe(downloadToLocal);
});

test("缺少个人词库导入能力时在下载调用处拒绝", async () => {
  const client = createDesktopCloudDictionaryClient(panelClient(), "ios", {});
  await expect(client.downloadToLocal!(entry)).rejects.toThrow(
    "personal dictionary import is unavailable",
  );
});

test("下载等待实际导入完成并透传导入失败", async () => {
  const failure = new Error("合成导入失败");
  let rejectImport!: (error: Error) => void;
  const pending = new Promise<never>((_resolve, reject) => (rejectImport = reject));
  const client = createDesktopCloudDictionaryClient(panelClient(), "android", {
    importPersonal: () => pending,
  });
  let settled = false;
  const download = client.downloadToLocal!(entry).finally(() => (settled = true));
  await Promise.resolve();
  expect(settled).toBe(false);
  const assertion = expect(download).rejects.toBe(failure);
  rejectImport(failure);
  await assertion;
  expect(settled).toBe(true);
});
