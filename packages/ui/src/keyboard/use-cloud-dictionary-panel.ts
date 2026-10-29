import { useEffect, useState } from "react";
import { useConfirm } from "../core/confirm";
import { useLatestRef } from "../core/use-latest-ref";
import { usePanelAction } from "./use-panel-action";
import { cloudDictionaryErrorMessage } from "./cloud-dictionary-errors";
import { cloudDictionaryEntries, cloudResponseInteger } from "./cloud-response";
import type { CloudDictionaryEntryFormValue } from "./cloud-dictionary-entry-form";
import type {
  CloudDictionaryKind,
  CloudDictionaryEntry,
  CloudDictionaryPanelClient,
} from "./panel-client";

/** 管理云词库列表、编辑状态和带代次保护的面板交互。 */
export function useCloudDictionaryPanel(client: CloudDictionaryPanelClient, embedded = false) {
  const { confirm, dismiss, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [search, setSearch] = useState("");
  const [offset, setOffset] = useState(0);
  const [entries, setEntries] = useState<CloudDictionaryEntry[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadError, setLoadError] = useState(false);
  const [form, setForm] = useState<{
    entry: CloudDictionaryEntry | null;
    code: string;
    word: string;
    weight: number;
  } | null>(null);
  const [notice, setNotice] = useState("管理当前账号的云端词条");
  const { busy, busyRef, revisionRef, run, invalidate, isCurrent } = usePanelAction(
    setNotice,
    cloudDictionaryErrorMessage,
  );
  const searchRef = useLatestRef(search);

  async function load(
    revision: number,
    nextOffset: number,
    nextSearch: string,
    nextKind: CloudDictionaryKind,
  ) {
    let result;
    try {
      result = await client.request({
        operation: "list",
        kind: nextKind,
        offset: nextOffset,
        search: nextSearch,
      });
    } catch (error) {
      if (isCurrent(revision)) {
        setEntries([]);
        setHasMore(false);
        setForm(null);
      }
      throw error;
    }
    if (!isCurrent(revision)) return;
    setEntries(cloudDictionaryEntries<CloudDictionaryEntry>(result));
    setOffset(cloudResponseInteger(result.offset) ? result.offset : nextOffset);
    setHasMore(result.has_more === true);
  }

  function refresh(nextOffset = 0, nextSearch = searchRef.current, nextKind = kind) {
    return run(async (revision) => {
      try {
        await load(revision, nextOffset, nextSearch, nextKind);
        if (isCurrent(revision)) {
          setLoadError(false);
          setNotice("云词库已刷新");
        }
      } catch (error) {
        if (isCurrent(revision)) {
          setLoadError(true);
          setNotice(cloudDictionaryErrorMessage(error, "无法加载云词库，请检查网络后重试。"));
        }
      }
    }, "无法加载云词库，请检查网络后重试。");
  }

  useEffect(() => {
    invalidate();
    dismiss();
    setEntries([]);
    setOffset(0);
    setHasMore(false);
    setForm(null);
    const timer = window.setTimeout(() => void refresh(0), embedded ? 280 : 0);
    return () => {
      window.clearTimeout(timer);
      invalidate();
    };
  }, [client, dismiss, invalidate, embedded, embedded ? kind : null, embedded ? search : null]);

  function beginAdd() {
    if (!busyRef.current) setForm({ entry: null, code: "", word: "", weight: 100000 });
  }
  function beginEdit(entry: CloudDictionaryEntry) {
    if (!busyRef.current)
      setForm({ entry, code: entry.code, word: entry.word, weight: entry.weight });
  }

  async function reloadAfterMutation(revision: number, success: string) {
    if (!isCurrent(revision)) return;
    try {
      await load(revision, 0, searchRef.current, kind);
      if (isCurrent(revision)) setNotice(success);
    } catch {
      if (isCurrent(revision)) {
        setEntries([]);
        setOffset(0);
        setHasMore(false);
        setNotice(`${success}，但列表刷新失败，请重新查询`);
      }
    }
  }

  function save() {
    if (busyRef.current) return;
    if (!form || !form.code.trim() || !form.word.trim()) {
      setNotice("编码和词条不能为空");
      return;
    }
    return run(async (revision) => {
      const code = form.code.trim();
      const word = form.word;
      if (form.entry) {
        await client.request({
          operation: "update",
          kind,
          id: form.entry.id,
          code,
          word,
          weight: form.weight,
          revision: form.entry.revision,
        });
      } else {
        await client.request({ operation: "add", kind, code, word, weight: form.weight });
      }
      if (!isCurrent(revision)) return;
      setForm(null);
      await reloadAfterMutation(revision, "云词条已保存");
    }, "云词条保存失败，请刷新后重试");
  }

  function remove(entry: CloudDictionaryEntry) {
    return run(async (revision) => {
      await client.request({ operation: "delete", kind, id: entry.id, revision: entry.revision });
      if (!isCurrent(revision)) return;
      setForm((current) => (current?.entry?.id === entry.id ? null : current));
      await reloadAfterMutation(revision, "云词条已删除");
    }, "云词条删除失败，请刷新后重试");
  }

  async function confirmRemove(entry: CloudDictionaryEntry) {
    if (busyRef.current) return;
    const revision = revisionRef.current;
    const confirmed = await confirm({
      title: "删除云词条",
      message: `“${entry.word}”只会从云端删除，本机词库不会改变。`,
      confirmLabel: "删除",
      danger: true,
    });
    // 确认期间可能更换客户端或开始其他操作，回答后重新核对代次和互斥。
    if (!confirmed || !isCurrent(revision) || busyRef.current) return;
    void remove(entry);
  }

  async function downloadToLocal(entry: CloudDictionaryEntry) {
    if (busyRef.current || !client.downloadToLocal) return;
    const revision = revisionRef.current;
    const confirmed = await confirm({
      title: "加入本机词典",
      message: `云词条“${entry.word}”会被加入本机个人词典。`,
      confirmLabel: "加入",
    });
    if (!confirmed || !isCurrent(revision) || busyRef.current || !client.downloadToLocal) return;
    const download = client.downloadToLocal;
    return run(async (revision) => {
      await download(entry);
      if (isCurrent(revision)) setNotice("云词条已加入本机词典队列");
    }, "加入本机词典失败，请重试");
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setForm(null);
    setOffset(0);
    setEntries([]);
    setHasMore(false);
    if (!embedded) void refresh(0, searchRef.current, next);
  }
  function changeForm(patch: Partial<CloudDictionaryEntryFormValue>) {
    setForm((current) => (current ? { ...current, ...patch } : null));
  }

  function cancelForm() {
    setForm(null);
  }

  return {
    confirmation,
    kind,
    search,
    setSearch,
    offset,
    entries,
    hasMore,
    loadError,
    form,
    notice,
    busy,
    refresh,
    beginAdd,
    beginEdit,
    save,
    confirmRemove,
    downloadToLocal,
    changeKind,
    changeForm,
    cancelForm,
  };
}

export type CloudDictionaryPanelState = ReturnType<typeof useCloudDictionaryPanel>;
