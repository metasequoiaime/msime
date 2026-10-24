import { useEffect, useRef, useState } from "react";
import { useDebouncedCallback } from "../core/use-debounced-callback";
import { useLatestRef } from "../core/use-latest-ref";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { normalizePinyinCode, validPinyinCode } from "./dictionary-collections";
import type { DictionaryClient } from "./dictionary-contracts";
import type { DictionaryEntry } from "./dictionary-file";

/** 内置词库详情页一次读的词条数，与 Android 的词库详情页相同。 */
const ENTRY_PAGE_SIZE = 100;

export interface BuiltinDictionaryEntriesOptions {
  dictionary: Pick<DictionaryClient, "list">;
  onNotice?: (message: string) => void;
}

/** 持有内置拼音词条的搜索、分页和请求归属，不依赖详情界面或提示容器。 */
export function useBuiltinDictionaryEntries({
  dictionary,
  onNotice,
}: BuiltinDictionaryEntriesOptions) {
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [more, setMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const generation = useAsyncGeneration(dictionary, query);
  const nextOffset = useRef(0);
  const notice = useLatestRef(onNotice);

  useEffect(() => {
    nextOffset.current = 0;
  }, [dictionary, query]);

  const load = async (offset: number) => {
    const current = ++generation.current;
    setLoading(true);
    const code = normalizePinyinCode(query);
    const byCode = query.trim() !== "" && validPinyinCode(code);
    try {
      const page = await dictionary.list(
        offset,
        ENTRY_PAGE_SIZE,
        "pinyin",
        byCode ? code : undefined,
      );
      if (current !== generation.current) return;
      // 不像拼音的搜索词按词语过滤已读出的词条，与 Android 的词库详情页相同。
      const found =
        query.trim() !== "" && !byCode
          ? page.entries.filter((entry) => entry.value.includes(query.trim()))
          : page.entries;
      nextOffset.current = offset + page.entries.length;
      setEntries((value) => (offset === 0 ? found : [...value, ...found]));
      setMore(page.has_more);
    } catch {
      if (current === generation.current) notice.current?.("读取词条失败，请稍后重试。");
    } finally {
      if (current === generation.current) setLoading(false);
    }
  };
  const { schedule: loadLater } = useDebouncedCallback(() => void load(0), query ? 250 : 0);
  useEffect(() => {
    loadLater();
  }, [loadLater, query, dictionary]);

  return {
    query,
    setQuery,
    entries,
    more,
    loading,
    reload: () => load(0),
    loadMore: () => load(nextOffset.current),
  };
}
