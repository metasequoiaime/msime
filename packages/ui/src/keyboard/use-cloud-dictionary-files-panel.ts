import { useEffect, useState } from "react";
import { useConfirm } from "../core/confirm";
import { useMountedRef } from "../settings/use-mounted-ref";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { usePanelAction } from "./use-panel-action";
import { cloudDictionaryErrorMessage } from "./cloud-dictionary-errors";
import { cloudResponseText } from "./cloud-response";
import { readDictionaryFile } from "../dictionary/dictionary-file";
import { downloadBlob } from "../core/download-blob";
import type {
  CloudDictionaryKind,
  CloudDictionaryFileFormat,
  CloudDictionarySnapshotMetadata,
  CloudDictionaryPanelClient,
} from "./panel-client";

/** 管理云词库文件传输、完整快照预览和恢复，并隔离失效宿主的异步结果。 */
export function useCloudDictionaryFilesPanel(client: CloudDictionaryPanelClient) {
  const { confirm, dismiss, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [format, setFormat] = useState<CloudDictionaryFileFormat>("standard");
  const [file, setFile] = useState<{ name: string; text: string; bytes: number } | null>(null);
  const [notice, setNotice] = useState("选择 TSV 文件后确认上传；导出不会改变云端内容");
  const {
    busy,
    busyRef,
    revisionRef: requestRevision,
    run,
    invalidate,
  } = usePanelAction(setNotice, cloudDictionaryErrorMessage);
  const [snapshotBusy, setSnapshotBusy] = useState(false);
  const [restorePreview, setRestorePreview] = useState<{
    text: string;
    snapshot: CloudDictionarySnapshotMetadata;
    expectedRevision: number;
  } | null>(null);
  const mounted = useMountedRef();
  const lifecycleRevision = useAsyncGeneration(client);

  useEffect(() => {
    invalidate();
    dismiss();
    setSnapshotBusy(false);
    setRestorePreview(null);
    return () => {
      invalidate();
      if (client.snapshotNative) void client.request({ operation: "snapshot_restore_cancel" });
    };
  }, [client, dismiss, invalidate]);

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setFile(null);
    if (next !== "pinyin" && format === "hans") setFormat("standard");
  }

  function chooseFile(selected: File) {
    if (busyRef.current) return;
    setFile(null);
    if (selected.size === 0 || selected.size > 65536) {
      setNotice("导入文件必须大于 0 且不超过 64 KiB");
      return;
    }
    void run(async (revision) => {
      let text: string;
      try {
        // 与本地词库导入使用同一个读取器。`File.text()` 只按 UTF-8 解码，
        // Windows 工具仍会写出的 GB18030 会变成替换字符并通过 NUL 检查，随后把错误词库上传到云端。
        // 这里沿用本地读取路径，两个入口对同一个文件保持一致。
        text = await readDictionaryFile(selected, 65536);
      } catch {
        throw new Error("invalid file");
      }
      if (revision !== requestRevision.current) return;
      if (!text || text.includes("\u0000")) throw new Error("invalid file");
      setFile({ name: selected.name || "dictionary.tsv", text, bytes: selected.size });
      setNotice("文件已读取；确认后才会写入云端");
    }, "无法读取文件，请确认是大小不超过 64 KiB 的文本词库");
  }

  async function importFile() {
    const prepared = file;
    if (!prepared || busyRef.current) return;
    const formatName =
      format === "hans" ? "汉字自动注音" : format === "windows" ? "Windows TSV" : "标准 TSV";
    const kindName =
      kind === "pinyin"
        ? "拼音"
        : kind === "wubi"
          ? "86 五笔"
          : kind === "wubi98"
            ? "98 五笔"
            : kind === "quick"
              ? "快捷短语"
              : "英文";
    const confirmed = await confirm({
      title: "导入词库",
      message: `按“${formatName}”导入${kindName}词库。`,
      confirmLabel: "导入",
    });
    if (!confirmed || busyRef.current) return;
    void run(async (revision) => {
      await client.request({ operation: "import", kind, format, text: prepared.text });
      if (revision !== requestRevision.current) return;
      setFile(null);
      setNotice("云词库已导入；如需影响本机输入，请另行应用到本机");
    }, "导入失败，请检查 TSV 文件格式");
  }

  function exportDictionary() {
    if (format === "hans") return;
    void run(async (revision) => {
      const result = await client.request({ operation: "export", kind, format });
      if (revision !== requestRevision.current) return;
      if (client.exportNative) {
        setNotice(result.saved === true ? "云词库已导出" : "已取消导出");
        return;
      }
      const text = cloudResponseText(result);
      if (typeof text !== "string") throw new Error("provider returned no file");
      const filename = result.filename || `msime-${kind}-dictionary.tsv`;
      if (client.saveExport) await client.saveExport(filename, text);
      else downloadBlob(new Blob([text], { type: "text/plain;charset=utf-8" }), filename);
      if (revision !== requestRevision.current) return;
      setNotice("云词库已导出");
    }, "导出失败，请检查网络和登录状态后重试");
  }

  async function exportSnapshot() {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setSnapshotBusy(true);
    try {
      const result = await client.request({ operation: "snapshot_export" });
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      if (client.snapshotNative && typeof result.saved === "boolean") {
        setNotice(result.saved ? "完整云词库快照已导出" : "已取消导出");
        return;
      }
      const text = cloudResponseText(result);
      if (typeof text !== "string" || !text) throw new Error("provider returned no snapshot");
      const filename = result.filename || "msime-dictionary-snapshot.ndjson";
      if (client.saveExport) await client.saveExport(filename, text);
      else downloadBlob(new Blob([text], { type: "application/x-ndjson;charset=utf-8" }), filename);
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      setNotice("完整云词库快照已导出");
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("导出完整云词库快照失败");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  async function chooseRestoreSnapshot(selected: File) {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setRestorePreview(null);
    if (selected.size === 0 || selected.size > 512 * 1024 * 1024) {
      setNotice("快照文件必须大于 0 且不超过 512 MiB");
      return;
    }
    setSnapshotBusy(true);
    try {
      const text = client.snapshotNative ? "" : await selected.text();
      const result = await client.request({ operation: "snapshot_restore_preview", text });
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      if (!result.snapshot || typeof result.expectedRevision !== "number")
        throw new Error("invalid snapshot preview");
      if (client.snapshotNative && typeof result.previewToken !== "string")
        throw new Error("invalid native snapshot preview");
      const preparedText = client.snapshotNative ? result.previewToken! : text;
      setRestorePreview({
        text: preparedText,
        snapshot: result.snapshot,
        expectedRevision: result.expectedRevision,
      });
      setNotice("快照已校验，请确认后替换云端词库");
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("无法校验快照，云端词库未改变");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  async function chooseNativeRestoreSnapshot() {
    if (!client.chooseSnapshotRestore || !mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setRestorePreview(null);
    setSnapshotBusy(true);
    try {
      const result = await client.chooseSnapshotRestore();
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      if (result.saved === false) {
        setNotice("已取消选择快照");
        return;
      }
      if (
        !result.snapshot ||
        typeof result.expectedRevision !== "number" ||
        typeof result.previewToken !== "string"
      )
        throw new Error("invalid native snapshot preview");
      setRestorePreview({
        text: result.previewToken,
        snapshot: result.snapshot,
        expectedRevision: result.expectedRevision,
      });
      setNotice("快照已校验，请确认后替换云端词库");
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("无法校验快照，云端词库未改变");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  async function abandonRestoreSnapshot() {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    setRestorePreview(null);
    if (client.snapshotNative) {
      setSnapshotBusy(true);
      try {
        await client.request({ operation: "snapshot_restore_cancel" });
      } finally {
        if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
      }
    }
  }

  async function restoreSnapshot() {
    if (!mounted.current) return;
    const lifecycle = lifecycleRevision.current;
    const prepared = restorePreview;
    if (!prepared) return;
    const confirmed = await confirm({
      title: "替换云端词库",
      message: "此快照会替换全部云端词库和排序记录。",
      confirmLabel: "替换",
      danger: true,
    });
    if (!confirmed || !mounted.current || lifecycle !== lifecycleRevision.current) return;
    setSnapshotBusy(true);
    try {
      const result = await client.request(
        client.snapshotNative
          ? { operation: "snapshot_restore_native", token: prepared.text }
          : {
              operation: "snapshot_restore",
              text: prepared.text,
              expected_sha256: prepared.snapshot.sha256,
              revision: prepared.expectedRevision,
            },
      );
      if (!mounted.current || lifecycle !== lifecycleRevision.current) return;
      setRestorePreview(null);
      setNotice(`云端词库已恢复到新版本 ${result.revision ?? ""}`.trim());
    } catch {
      if (mounted.current && lifecycle === lifecycleRevision.current)
        setNotice("恢复失败，可能是云端版本已变化；云端词库未改变");
    } finally {
      if (mounted.current && lifecycle === lifecycleRevision.current) setSnapshotBusy(false);
    }
  }

  return {
    confirmation,
    kind,
    format,
    file,
    setFile,
    notice,
    busy,
    snapshotBusy,
    restorePreview,
    changeKind,
    setFormat,
    chooseFile,
    importFile,
    exportDictionary,
    exportSnapshot,
    chooseRestoreSnapshot,
    chooseNativeRestoreSnapshot,
    abandonRestoreSnapshot,
    restoreSnapshot,
  };
}

export type CloudDictionaryFilesPanelState = ReturnType<typeof useCloudDictionaryFilesPanel>;
