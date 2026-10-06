import { useEffect, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { randomRequestId } from "../core/random-id";
import {
  DICTIONARY_PAGE_SIZE,
  dictionaryPageStatus,
  readDictionaryFile,
  type DictionaryEntry,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
} from "../dictionary/dictionary-file";
import { describeImportResult } from "../dictionary/dictionary-messages";
import {
  dictionaryExportName,
  dictionaryExportPayload,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  dictionaryKindLabel,
} from "../dictionary/dictionary-export";
import {
  dictionaryErrorMessage,
  dictionaryKeyMatches,
  importFailureMessage,
} from "../dictionary/dictionary-errors";
import type { DictionaryClient, DictionaryFailure, DictionaryImportResult } from "../index";
import type { DictionaryPhraseForm } from "./dictionary-entries";
import { useMountedRef } from "./use-mounted-ref";
import { useAsyncGeneration } from "./use-async-generation";

export interface DictionaryManagerClient {
  dictionary?: DictionaryClient;
  resetLearnedData?: () => Promise<void>;
  saveExport?: (name: string, contents: string) => Promise<string | null>;
}

export interface DictionaryConfirmOptions {
  title: string;
  message: string;
  confirmLabel: string;
  danger?: boolean;
}

export interface UseDictionaryManagerOptions {
  client: DictionaryManagerClient;
  confirm: (options: DictionaryConfirmOptions) => Promise<boolean>;
}

export function useDictionaryManager({ client, confirm }: UseDictionaryManagerOptions) {
  const [phrases, setPhrases] = useState<DictionaryEntry[]>([]);
  const [phrasePage, setPhrasePage] = useState({ offset: 0, hasMore: false, status: "" });
  const [phraseBusy, setPhraseBusy] = useState(false);
  const [phraseError, setPhraseError] = useState("");
  const [dictionaryPendingCount, setDictionaryPendingCount] = useState(0);
  const [dictionaryFailures, setDictionaryFailures] = useState<DictionaryFailure[]>([]);
  const [dictionarySnapshotError, setDictionarySnapshotError] = useState("");
  const [phraseNotice, setPhraseNotice] = useState("");
  const [phraseSearch, setPhraseSearch] = useState("");
  const [phraseForm, setPhraseForm] = useState<DictionaryPhraseForm | null>(null);
  const [dictionaryKind, setDictionaryKind] = useState<LocalDictionaryKind>("quick_phrase");
  const [dictionaryFormat, setDictionaryFormat] = useState<LocalDictionaryFormat>("standard");
  const phraseRequestGeneration = useAsyncGeneration(client.dictionary);
  const phraseActionBusy = useRef(false);
  const phraseActionOwner = useAsyncGeneration();
  const phraseListRef = useRef<HTMLUListElement>(null);
  const mounted = useMountedRef();
  const clientGeneration = useAsyncGeneration(
    client.dictionary,
    client.resetLearnedData,
    client.saveExport,
  );

  useEffect(() => {
    phraseActionOwner.current += 1;
    phraseActionBusy.current = false;
    setPhraseBusy(false);
  }, [client.dictionary]);

  useEffect(() => {
    phraseActionOwner.current += 1;
    phraseActionBusy.current = false;
    setPhraseBusy(false);
  }, [client.dictionary, client.resetLearnedData, client.saveExport]);

  async function runPhraseAction(
    operation: (isCurrent: () => boolean) => Promise<void>,
    formatError: (error: unknown) => string,
    generation = clientGeneration.current,
  ) {
    if (phraseActionBusy.current || phraseBusy) return;
    const owner = ++phraseActionOwner.current;
    phraseActionBusy.current = true;
    await runAsyncAction(
      {
        busy: false,
        isCurrent: () =>
          mounted.current &&
          generation === clientGeneration.current &&
          owner === phraseActionOwner.current,
        setBusy: (busy) => {
          if (owner !== phraseActionOwner.current) return;
          phraseActionBusy.current = busy;
          setPhraseBusy(busy);
        },
        setError: setPhraseError,
        setNotice: setPhraseNotice,
      },
      operation,
      { formatError },
    );
    if (owner === phraseActionOwner.current) phraseActionBusy.current = false;
  }

  async function loadPhrases(kind: LocalDictionaryKind = dictionaryKind, offset = 0) {
    if (!client.dictionary || !mounted.current) return;
    const generation = ++phraseRequestGeneration.current;
    setPhraseBusy(true);
    setPhraseError("");
    setPhraseNotice("");
    setPhrasePage((current) => ({ ...current, status: "查询中…" }));
    try {
      const query = phraseSearch.trim();
      const page = await client.dictionary.list(offset, DICTIONARY_PAGE_SIZE, kind, query);
      if (!mounted.current || generation !== phraseRequestGeneration.current) return;
      const entries = page.entries.filter(
        (entry) => entry.kind === kind && dictionaryKeyMatches(kind, entry.key, query),
      );
      setPhrases(entries);
      setDictionaryPendingCount(page.pending_count ?? 0);
      setDictionaryFailures(page.failed_requests ?? []);
      setDictionarySnapshotError(page.snapshot_error ?? "");
      setPhrasePage({
        offset,
        hasMore: page.has_more && page.entries.length > 0,
        status: dictionaryPageStatus(offset, entries.length, page.has_more),
      });
    } catch {
      if (!mounted.current || generation !== phraseRequestGeneration.current) return;
      setPhraseError("无法读取词库。");
      setPhrasePage((current) => ({ ...current, status: "查询失败，请重试" }));
    } finally {
      if (mounted.current && generation === phraseRequestGeneration.current) setPhraseBusy(false);
    }
  }

  function turnPhrasePage(offset: number) {
    if (phraseListRef.current) phraseListRef.current.scrollTop = 0;
    void loadPhrases(dictionaryKind, offset);
  }

  async function removePhrase(entry: DictionaryEntry) {
    if (!client.dictionary || !mounted.current) return;
    const generation = clientGeneration.current;
    const dictionary = client.dictionary;
    const confirmed = await confirm({
      title: "删除词条",
      message: `“${entry.value}”（${entry.key}）将被删除，此操作无法撤销。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed || !mounted.current || clientGeneration.current !== generation) return;
    await runPhraseAction(
      async (isCurrent) => {
        await dictionary.edit(entry, null, randomRequestId("ui-remove"));
        if (!isCurrent()) return;
        const remaining = phrases.length - 1;
        const offset =
          remaining === 0 && phrasePage.offset > 0
            ? Math.max(0, phrasePage.offset - DICTIONARY_PAGE_SIZE)
            : phrasePage.offset;
        await loadPhrases(dictionaryKind, offset);
      },
      (error) =>
        dictionaryErrorMessage(
          error,
          `${dictionaryKindLabel(dictionaryKind)}删除失败，请稍后重试。`,
        ),
      generation,
    );
  }

  async function savePhrase() {
    if (!client.dictionary || !phraseForm || !mounted.current) return;
    const generation = clientGeneration.current;
    const dictionary = client.dictionary;
    const form = phraseForm;
    const bundled = form.previous?.source === "bundled" ? form.previous : null;
    const replacement: DictionaryEntry = bundled
      ? { ...bundled, weight: form.weight }
      : {
          kind: dictionaryKind,
          key: form.key.trim(),
          value: form.value,
          weight: form.weight,
        };
    if (!replacement.key || !replacement.value) {
      setPhraseError("编码和短语不能为空。");
      return;
    }
    await runPhraseAction(
      async (isCurrent) => {
        await dictionary.edit(
          form.previous,
          replacement,
          randomRequestId(form.previous ? "ui-edit" : "ui-add"),
        );
        if (!isCurrent()) return;
        setPhraseForm(null);
        await loadPhrases(dictionaryKind, form.previous ? phrasePage.offset : 0);
      },
      (error) =>
        dictionaryErrorMessage(
          error,
          `${dictionaryKindLabel(dictionaryKind)}保存失败，请稍后重试。`,
          dictionaryKind,
        ),
      generation,
    );
  }

  async function importPhrases(file: File) {
    if (!client.dictionary || !mounted.current) return;
    const generation = clientGeneration.current;
    const dictionary = client.dictionary;
    await runPhraseAction(
      async (isCurrent) => {
        const text = await readDictionaryFile(file, dictionary.maxImportFileBytes);
        let imported: DictionaryImportResult | null = null;
        if (dictionary.import) {
          imported = await dictionary.import(
            dictionaryKind,
            dictionaryFormat,
            text,
            randomRequestId("ui-import"),
          );
        } else {
          if (dictionaryFormat === "hans") throw new Error("hans format requires batch import");
          const lines = text.split(/\r?\n/).filter(Boolean);
          for (const line of lines) {
            const [first, second, weight = "10000"] = line.split("\t");
            if (!first || !second) continue;
            const [value, key] = dictionaryFormat === "windows" ? [second, first] : [first, second];
            const parsedWeight = Number(weight);
            const normalizedWeight =
              weight.trim() !== "" && Number.isSafeInteger(parsedWeight) && parsedWeight >= 0
                ? parsedWeight
                : 10000;
            await dictionary.edit(
              null,
              { kind: dictionaryKind, key: key.trim(), value, weight: normalizedWeight },
              randomRequestId("ui-import"),
            );
          }
        }
        if (!isCurrent()) return;
        await loadPhrases(dictionaryKind);
        if (!isCurrent()) return;
        if (imported)
          setPhraseNotice(describeImportResult(dictionaryKindLabel(dictionaryKind), imported));
      },
      (error) => importFailureMessage(dictionaryKindLabel(dictionaryKind), error),
      generation,
    );
  }

  async function retryDictionaryFailure(requestId: string) {
    const generation = clientGeneration.current;
    const retry = client.dictionary?.retry;
    if (!retry) return;
    await runPhraseAction(
      async (isCurrent) => {
        await retry(requestId);
        if (!isCurrent()) return;
        await loadPhrases(dictionaryKind, phrasePage.offset);
      },
      () => "词条重试失败，请稍后重试。",
      generation,
    );
  }

  async function dismissDictionaryFailure(requestId: string) {
    const generation = clientGeneration.current;
    const dismissFailure = client.dictionary?.dismissFailure;
    if (!dismissFailure) return;
    await runPhraseAction(
      async (isCurrent) => {
        await dismissFailure(requestId);
        if (!isCurrent()) return;
        await loadPhrases(dictionaryKind, phrasePage.offset);
      },
      () => "移除失败记录失败，请稍后重试。",
      generation,
    );
  }

  async function deliverDictionaryExport(
    name: string,
    body: string,
    generation = clientGeneration.current,
  ): Promise<string | null | undefined> {
    const isCurrent = () => mounted.current && generation === clientGeneration.current;
    if (!isCurrent()) return undefined;
    const saveExport = client.saveExport;
    if (saveExport) {
      let path: string | null;
      try {
        path = await saveExport(name, body);
      } catch (error) {
        if (!isCurrent()) return undefined;
        // A host that knows why its own save failed (the Harmony save picker) says so in an Error; anything else is the macOS Downloads write.
        setPhraseNotice("");
        setPhraseError(
          error instanceof Error && error.message
            ? error.message
            : "无法写入“下载”文件夹，词库未导出。",
        );
        return undefined;
      }
      if (!isCurrent()) return undefined;
      // A host with a save picker resolves null when the user closes it, which is neither a failure nor an export.
      if (path === null) {
        setPhraseNotice("已取消导出。");
        return undefined;
      }
      return path;
    }
    if (!isCurrent()) return undefined;
    const url = URL.createObjectURL(new Blob([body], { type: "text/plain;charset=utf-8" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    URL.revokeObjectURL(url);
    return null;
  }

  async function exportPhrases() {
    const dictionary = client.dictionary;
    if (!dictionary) return;
    const generation = clientGeneration.current;
    if (dictionaryFormat === "hans") {
      setPhraseError("汉字自动注音格式仅支持导入。");
      return;
    }
    await runPhraseAction(
      async (isCurrent) => {
        let text = "";
        if (dictionary.export) {
          let offset = 0;
          let hasMore = true;
          while (hasMore && offset <= 1000000) {
            const page = await dictionary.export(
              dictionaryKind,
              dictionaryFormat === "rime" ? "standard" : dictionaryFormat,
              offset,
              1000,
            );
            text += page.text;
            const count = page.text ? page.text.trimEnd().split("\n").length : 0;
            offset += count;
            hasMore = page.has_more && count > 0;
          }
        } else {
          text = phrases
            .map((entry) =>
              dictionaryFormat === "windows"
                ? `${entry.key}\t${entry.value}\t${entry.weight}`
                : `${entry.value}\t${entry.key}\t${entry.weight}`,
            )
            .join("\n");
        }
        if (!isCurrent()) return;
        const payload = dictionaryExportPayload(dictionaryKind, dictionaryFormat, text);
        if (!payload.rows) {
          setPhraseError("当前没有可导出的用户新增词条。");
          return;
        }
        const path = await deliverDictionaryExport(
          dictionaryExportName(dictionaryKind),
          payload.body,
          generation,
        );
        if (path === undefined || !isCurrent()) return;
        setPhraseNotice(
          path === null
            ? `已导出 ${payload.rows} 条用户词条。`
            : `已导出 ${payload.rows} 条用户词条到 ${path}。`,
        );
      },
      (error) => dictionaryErrorMessage(error, "词库导出失败，请稍后重试。"),
    );
  }

  async function exportAllPhrases() {
    const dictionary = client.dictionary;
    if (!dictionary) return;
    const generation = clientGeneration.current;
    await runPhraseAction(
      async (isCurrent) => {
        setPhraseNotice("正在读取全部用户词库…");
        const payload = personalDictionaryExportPayload(
          await loadAllPersonalDictionaryEntries(dictionary),
        );
        if (!isCurrent()) return;
        if (!payload.rows) {
          setPhraseNotice("当前没有可导出的用户词条。");
          return;
        }
        const path = await deliverDictionaryExport(
          personalDictionaryExportName(),
          payload.body,
          generation,
        );
        if (path === undefined || !isCurrent()) return;
        setPhraseNotice(
          path === null
            ? `已导出全部 ${payload.rows} 条用户词条。`
            : `已导出全部 ${payload.rows} 条用户词条到 ${path}。`,
        );
      },
      (error) => dictionaryErrorMessage(error, "全部词库导出失败，请稍后重试。"),
    );
  }

  async function resetLearnedData() {
    if (!client.resetLearnedData || phraseBusy || phraseActionBusy.current) return;
    const generation = clientGeneration.current;
    const reset = client.resetLearnedData;
    const confirmed = await confirm({
      title: "清除学习数据",
      message:
        "候选词频、用户词库（包括自己新增和修改的词条）和拼音学习记录将永久删除，此操作无法撤销。输入方案等设置不会改变。",
      confirmLabel: "清除",
      danger: true,
    });
    if (!confirmed || !mounted.current || clientGeneration.current !== generation) return;
    await runPhraseAction(
      async (isCurrent) => {
        await reset();
        if (!isCurrent()) return;
        setPhrases([]);
        setPhrasePage({ offset: 0, hasMore: false, status: "已清除学习数据" });
        setDictionaryPendingCount(0);
        setDictionaryFailures([]);
        setDictionarySnapshotError("");
        setPhraseNotice("已清除所有学习数据；输入方案和设置保持不变。");
      },
      (error) =>
        dictionaryErrorMessage(error, "清除学习数据失败，请关闭正在使用输入法的程序后重试。"),
      generation,
    );
  }

  return {
    phrases,
    setPhrases,
    phrasePage,
    phraseBusy,
    phraseError,
    dictionaryPendingCount,
    dictionaryFailures,
    dictionarySnapshotError,
    phraseNotice,
    phraseSearch,
    setPhraseSearch,
    phraseForm,
    setPhraseForm,
    dictionaryKind,
    setDictionaryKind,
    dictionaryFormat,
    setDictionaryFormat,
    phraseListRef,
    loadPhrases,
    turnPhrasePage,
    removePhrase,
    savePhrase,
    importPhrases,
    retryDictionaryFailure,
    dismissDictionaryFailure,
    exportPhrases,
    exportAllPhrases,
    resetLearnedData,
  };
}
