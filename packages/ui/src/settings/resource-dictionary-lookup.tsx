import { useEffect, useRef, useState } from "react";
import type { DictionaryClient } from "../dictionary/dictionary-contracts";
import {
  DICTIONARY_PAGE_SIZE,
  dictionaryPageStatus,
  type DictionaryLookupEntry,
  type ResourceDictionaryKind,
} from "../dictionary/dictionary-file";
import { resourceDictionaryKinds } from "../dictionary/dictionary-kinds";
import { dictionaryErrorMessage } from "../dictionary/dictionary-errors";
import { DictionaryEntryList } from "./dictionary-entries";
import { DictionaryPagination } from "./dictionary-pagination";
import { TextInputRow } from "./text-input-row";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsInputDescription } from "./settings-input-description";
import { ErrorAlert } from "../core/error-alert";
import { useDebouncedCallback } from "../core/use-debounced-callback";
import * as settings from "./settings-style";

const placeholders: Record<ResourceDictionaryKind, string> = {
  japanese: "例如：日本、にほん 或 ニホン",
  korean_hanja: "输入韩文读音或汉字，例如：한 或 韓",
  cantonese: "输入词条或粤拼，例如：你好 或 nei",
  zhuyin: "输入词条或注音，例如：你好 或 ㄋㄧ",
  stroke: "输入汉字或笔画编码，例如：一 或 h",
};

/** 语言词库直接在宿主筛选和分页；不经过个人词条的编码校验与写入路径。 */
export function ResourceDictionaryLookup({
  dictionary,
  kind,
}: {
  dictionary: DictionaryClient;
  kind: ResourceDictionaryKind;
}) {
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [result, setResult] = useState({
    entries: [] as DictionaryLookupEntry[],
    has_more: false,
    available: true,
  });
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const listRef = useRef<HTMLUListElement>(null);

  const { schedule: browseLater, cancel: cancelBrowse } = useDebouncedCallback(
    async (isCurrent: () => boolean) => {
      try {
        if (!dictionary.browse) return;
        const page = await dictionary.browse(offset, DICTIONARY_PAGE_SIZE, kind, query.trim());
        if (isCurrent()) {
          setResult({ ...page, entries: page.entries.filter((entry) => entry.kind === kind) });
          if (listRef.current) listRef.current.scrollTop = 0;
        }
      } catch (reason) {
        if (isCurrent()) {
          setResult({ entries: [], has_more: false, available: true });
          setError(dictionaryErrorMessage(reason, "无法读取词库，请重试。"));
        }
      } finally {
        if (isCurrent()) setBusy(false);
      }
    },
    280,
  );

  useEffect(() => {
    let current = true;
    setBusy(true);
    setError("");
    browseLater(() => current);
    return () => {
      current = false;
      cancelBrowse();
    };
  }, [dictionary, kind, query, offset, browseLater, cancelBrowse]);

  const label = resourceDictionaryKinds.find(([value]) => value === kind)?.[1];
  const status = busy
    ? "查询中…"
    : error
      ? "查询失败"
      : !result.available
        ? `${label}词库尚未安装，请在「输入」设置中下载对应的词库资源。`
        : dictionaryPageStatus(offset, result.entries.length, result.has_more);

  return (
    <>
      <TextInputRow
        title="词条或编码"
        label="词条或编码"
        className={settings.fieldInput}
        type="search"
        value={query}
        placeholder={placeholders[kind]}
        onChange={(value) => {
          setQuery(value);
          setOffset(0);
        }}
      />
      <SettingsManagerBlock>
        <SettingsInputDescription>
          基础词库只读
          {kind === "japanese"
            ? "，显示假名读音与词条"
            : kind === "korean_hanja"
              ? "，按内置汉字表顺序显示"
              : ""}
          。
        </SettingsInputDescription>
        {error && <ErrorAlert>{error}</ErrorAlert>}
        <DictionaryEntryList
          kind={kind}
          entries={result.entries}
          busy={busy}
          listRef={listRef}
          emptyMessage={status}
        />
        <div className={settings.dictionaryFooter}>
          <div className="ml-auto">
            <DictionaryPagination
              busy={busy}
              offset={offset}
              hasMore={result.has_more && result.available}
              status={status}
              pageSize={DICTIONARY_PAGE_SIZE}
              onPageChange={setOffset}
            />
          </div>
        </div>
      </SettingsManagerBlock>
    </>
  );
}
