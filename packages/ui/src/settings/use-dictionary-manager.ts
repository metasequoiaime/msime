import { downloadBlob } from "../core/download-blob";
import { useEffect, useMemo, useRef, useState } from "react";
import { runAsyncAction } from "../core/async-action";
import { randomRequestId } from "../core/random-id";
import { utf8ByteLength } from "../core/text";
import { useDebouncedCallback } from "../core/use-debounced-callback";
import {
  DICTIONARY_PAGE_SIZE,
  MAX_DICTIONARY_FILE_BYTES,
  MAX_SCEL_FILE_BYTES,
  dictionaryPageStatus,
  isScelFile,
  leadingImportRows,
  readDictionaryFile,
  readScelFile,
  type DictionaryEntry,
  type DictionaryLookupEntry,
  type DictionaryPageFormat,
  type LocalDictionaryFormat,
  type LocalDictionaryKind,
} from "../dictionary/dictionary-file";
import {
  describeImportResult,
  SCEL_PERSONAL_USE_NOTICE,
  scelImportSource,
} from "../dictionary/dictionary-messages";
import {
  MAX_DICTIONARY_EXPORT_BYTES,
  dictionaryExportName,
  dictionaryExportPayload,
  personalDictionaryExportName,
  personalDictionaryExportPayload,
  loadAllPersonalDictionaryEntries,
  loadDictionaryExportText,
  dictionaryKindLabel,
} from "../dictionary/dictionary-export";
import {
  dictionaryErrorMessage,
  dictionaryEntryMatches,
  importFailureMessage,
} from "../dictionary/dictionary-errors";
import type {
  DictionaryClient,
  DictionaryFailure,
  DictionaryImportResult,
} from "../dictionary/dictionary-contracts";
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
  // 「清除全部学习数据」的结果单独存放：桌面把它画在页末「学习数据」组旁边，按钮就在那里，其余词库操作的结果画在页顶。
  const [learnedDataError, setLearnedDataError] = useState("");
  const [learnedDataNotice, setLearnedDataNotice] = useState("");
  const [phraseSearch, setPhraseSearch] = useState("");
  const [phraseForm, setPhraseForm] = useState<DictionaryPhraseForm | null>(null);
  const [dictionaryKind, setDictionaryKind] = useState<LocalDictionaryKind>("quick_phrase");
  const [dictionaryFormat, setDictionaryFormat] = useState<DictionaryPageFormat>("standard");
  const phraseQuery = useMemo(
    () => ({ dictionaryKind, phraseSearch }),
    [dictionaryKind, phraseSearch],
  );
  const currentPhraseQuery = useRef({
    query: phraseQuery,
    load: loadPhrases,
    offset: 0,
  });
  useEffect(() => {
    currentPhraseQuery.current = {
      query: phraseQuery,
      load: loadPhrases,
      // 新查询从首页开始，后续记录已经发出的页码，不能沿用上个查询尚未替换的显示页码。
      offset:
        currentPhraseQuery.current.query === phraseQuery ? currentPhraseQuery.current.offset : 0,
    };
  });
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

  /** 清掉词库操作留下的提示，包括清除学习数据的结果。查询条件变化、新操作开始时都会清；桌面切换词库页标签时也调用它，提示只留在产生它的那个标签上。 */
  function clearPhraseFeedback() {
    setPhraseError("");
    setPhraseNotice("");
    setLearnedDataError("");
    setLearnedDataNotice("");
  }

  async function runPhraseAction(
    operation: (isCurrent: () => boolean) => Promise<void>,
    formatError: (error: unknown) => string,
    generation = clientGeneration.current,
    feedback: { setError: (message: string) => void; setNotice: (message: string) => void } = {
      setError: setPhraseError,
      setNotice: setPhraseNotice,
    },
  ) {
    if (phraseActionBusy.current || phraseBusy) return;
    const owner = ++phraseActionOwner.current;
    phraseActionBusy.current = true;
    // 新操作开始时两处提示都清掉，页面上只留这一次操作的结果。
    clearPhraseFeedback();
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
        setError: feedback.setError,
        setNotice: feedback.setNotice,
      },
      operation,
      { formatError },
    );
    if (owner === phraseActionOwner.current) phraseActionBusy.current = false;
  }

  async function loadPhrases(
    kind: LocalDictionaryKind = dictionaryKind,
    offset = 0,
    options: { preserveError?: boolean } = {},
  ): Promise<void> {
    if (!client.dictionary || !mounted.current) return;
    // 旧回执仍需刷新写入后的数据，但必须使用当前查询和页码，避免旧条件赢得最新请求代次。
    const current = currentPhraseQuery.current;
    if (phraseQuery !== current.query) return current.load(undefined, current.offset, options);
    current.offset = offset;
    const generation = ++phraseRequestGeneration.current;
    setPhraseBusy(true);
    if (!options.preserveError) setPhraseError("");
    setPhrasePage((current) => ({ ...current, status: "查询中…" }));
    try {
      const query = phraseSearch.trim();
      // 新宿主的浏览入口允许空查询列出内置词条；旧宿主继续使用个人词库接口。
      const page = client.dictionary.browse
        ? {
            ...(await client.dictionary.browse(offset, DICTIONARY_PAGE_SIZE, kind, query)),
            pending_count: 0,
            failed_requests: [],
            snapshot_error: null,
          }
        : await client.dictionary.list(offset, DICTIONARY_PAGE_SIZE, kind, query);
      if (!mounted.current || generation !== phraseRequestGeneration.current) return;
      const listed: DictionaryLookupEntry[] = page.entries;
      const entries = listed.filter(
        (entry): entry is DictionaryEntry =>
          entry.kind === kind && dictionaryEntryMatches(entry as DictionaryEntry, query),
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

  const { schedule: loadPhrasesLater, cancel: cancelPhraseQuery } = useDebouncedCallback(
    (kind: LocalDictionaryKind) => void loadPhrases(kind, 0, { preserveError: true }),
    280,
  );

  // 进入设置时先列出当前类型的全部词条；输入查询条件后用短暂防抖实时筛选。
  useEffect(() => {
    if (!client.dictionary) return;
    // 查询条件变化时清理旧提示，延迟执行的查询不能擦掉期间完成的操作结果。
    clearPhraseFeedback();
    loadPhrasesLater(dictionaryKind);
    return () => {
      cancelPhraseQuery();
      // 查询条件已经变化时，旧响应在下一次防抖请求发出前也不能覆盖列表。
      phraseRequestGeneration.current += 1;
    };
    // 初次挂载、宿主替换、类型切换或输入变化都走同一条防抖查询路径。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [client.dictionary, dictionaryKind, phraseSearch, loadPhrasesLater, cancelPhraseQuery]);

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

  /**
   * 导入一个文件。`format` 由选文件的那一刻决定（例如按扩展名切到 `rime` 或 `scel`）：同一个事件里刚 `setDictionaryFormat` 的值要到下一次渲染才进闭包，所以由调用方直接传进来，不读闭包里的旧格式。
   *
   * 细胞词库（选了这个格式，或者文件开头就是细胞词库的魔数）先经宿主的 `scelText` 转成标准文本，再以 `standard` 交给 `import`，上限、分批和报告都与文本文件相同。不分批的宿主（设了 `maxImportFileBytes`）一次只收 64 KiB 文本、只看 1000 行，所以只送转出文本的开头，报告里说文件过长，而不是整份被拒绝。
   */
  async function importPhrases(file: File, format: DictionaryPageFormat = dictionaryFormat) {
    if (!client.dictionary || !mounted.current) return;
    const generation = clientGeneration.current;
    const dictionary = client.dictionary;
    await runPhraseAction(
      async (isCurrent) => {
        const scelText = dictionary.scelText;
        const scel = format === "scel" || (scelText !== undefined && (await isScelFile(file)));
        let text: string;
        let textFormat: LocalDictionaryFormat;
        let source = "";
        let cut = false;
        if (scel) {
          if (!scelText) throw new Error("这里不能导入搜狗细胞词库。");
          if (dictionaryKind !== "pinyin")
            throw new Error("搜狗细胞词库只能导入拼音词库，请先把「词库」切换到拼音。");
          const converted = await scelText(
            await readScelFile(
              file,
              Math.min(
                MAX_SCEL_FILE_BYTES,
                dictionary.maxImportFileBytes ?? MAX_DICTIONARY_FILE_BYTES,
              ),
            ),
          );
          text = converted.text;
          if (dictionary.maxImportFileBytes !== undefined)
            ({ text, cut } = leadingImportRows(converted.text));
          textFormat = "standard";
          source = scelImportSource(converted.name, converted.entries);
        } else {
          text = await readDictionaryFile(file, dictionary.maxImportFileBytes);
          textFormat = format;
        }
        let imported: DictionaryImportResult | null = null;
        if (dictionary.import) {
          imported = await dictionary.import(
            dictionaryKind,
            textFormat,
            text,
            randomRequestId("ui-import"),
          );
          if (cut) imported = { ...imported, truncated: true };
        } else {
          if (textFormat === "hans") throw new Error("hans format requires batch import");
          const lines = text.split(/\r?\n/).filter(Boolean);
          for (const line of lines) {
            const [first, second, weight = "10000"] = line.split("\t");
            if (!first || !second) continue;
            const [value, key] = textFormat === "windows" ? [second, first] : [first, second];
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
          setPhraseNotice(
            describeImportResult(dictionaryKindLabel(dictionaryKind), imported, source) +
              (scel ? SCEL_PERSONAL_USE_NOTICE : ""),
          );
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
    if (utf8ByteLength(body) > MAX_DICTIONARY_EXPORT_BYTES) {
      throw new Error("dictionary_export_limit");
    }
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
    downloadBlob(new Blob([body], { type: "text/plain;charset=utf-8" }), name);
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
    if (dictionaryFormat === "scel") {
      setPhraseError("搜狗细胞词库格式仅支持导入。");
      return;
    }
    await runPhraseAction(
      async (isCurrent) => {
        let text = "";
        if (dictionary.export) {
          text = await loadDictionaryExportText(
            dictionary,
            dictionaryKind,
            dictionaryFormat === "rime" ? "standard" : dictionaryFormat,
            MAX_DICTIONARY_EXPORT_BYTES,
          );
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
        setLearnedDataNotice("已清除所有学习数据；输入方案和设置保持不变。");
      },
      (error) =>
        dictionaryErrorMessage(error, "清除学习数据失败，请关闭正在使用输入法的程序后重试。"),
      generation,
      { setError: setLearnedDataError, setNotice: setLearnedDataNotice },
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
    learnedDataError,
    learnedDataNotice,
    clearPhraseFeedback,
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
