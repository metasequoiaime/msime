import { useEffect, useState } from "react";
import { useConfirm } from "../core/confirm";
import { usePanelAction } from "./use-panel-action";
import { cloudDictionaryErrorMessage } from "./cloud-dictionary-errors";
import { cloudDictionaryCatalogEntries, cloudResponseInteger } from "./cloud-response";
import type { CloudDictionaryEntryFormValue } from "./cloud-dictionary-entry-form";
import type {
  CloudDictionaryKind,
  CloudDictionaryCatalogEntry,
  CloudDictionaryPanelClient,
} from "./panel-client";

/** 管理完整云词库目录的查询、分页、编辑与带代次保护的删除确认。 */
export function useCloudDictionaryCatalogPanel(client: CloudDictionaryPanelClient) {
  const { confirm, dismiss, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [code, setCode] = useState("");
  const [scheme, setScheme] = useState("pinyin");
  const [profile, setProfile] = useState("xiaohe");
  const [confirmed, setConfirmed] = useState<{
    code: string;
    scheme: string;
    profile: string;
  } | null>(null);
  const [entries, setEntries] = useState<CloudDictionaryCatalogEntry[]>([]);
  const [offset, setOffset] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [revision, setRevision] = useState(0);
  const [normalized, setNormalized] = useState("");
  const [form, setForm] = useState<{
    entry: CloudDictionaryCatalogEntry;
    replacement: boolean;
    code: string;
    word: string;
    weight: number;
  } | null>(null);
  const [notice, setNotice] = useState("查询基础词库与当前账号的完整目录");
  const { busy, busyRef, revisionRef, run, invalidate, isCurrent } = usePanelAction(
    setNotice,
    cloudDictionaryErrorMessage,
  );

  useEffect(() => {
    invalidate();
    dismiss();
    setEntries([]);
    setForm(null);
    setConfirmed(null);
    setHasMore(false);
    setOffset(0);
    setRevision(0);
    setNormalized("");
    return () => {
      invalidate();
    };
  }, [client, dismiss, invalidate]);

  async function load(
    current: number,
    nextOffset: number,
    query: { code: string; scheme: string; profile: string },
  ) {
    let result;
    try {
      result = await client.request({
        operation: "catalog",
        kind,
        code: query.code,
        offset: nextOffset,
        scheme: query.scheme,
        profile: query.profile,
      });
    } catch (error) {
      if (isCurrent(current)) {
        setEntries([]);
        setForm(null);
        setConfirmed(null);
        setHasMore(false);
        setOffset(0);
        setRevision(0);
        setNormalized("");
      }
      throw error;
    }
    if (!isCurrent(current)) return;
    setEntries(cloudDictionaryCatalogEntries<CloudDictionaryCatalogEntry>(result));
    setOffset(cloudResponseInteger(result.offset) ? result.offset : nextOffset);
    setHasMore(result.has_more === true);
    setRevision(cloudResponseInteger(result.revision) ? result.revision : 0);
    setNormalized(typeof result.normalized === "string" ? result.normalized : query.code);
    setConfirmed(query);
  }

  function queryCatalog(
    nextOffset = 0,
    queryOverride?: { code: string; scheme: string; profile: string },
  ) {
    const query = queryOverride ?? { code: code.trim(), scheme, profile };
    if (kind !== "quick" && !query.code) {
      setNotice("请输入查询编码");
      return;
    }
    return run(async (current) => {
      await load(current, nextOffset, query);
      if (isCurrent(current)) setNotice("完整目录已刷新");
    }, "无法访问完整云词库目录，请确认账号已登录");
  }

  function beginEdit(entry: CloudDictionaryCatalogEntry) {
    if (!busyRef.current)
      setForm({
        entry,
        replacement: true,
        code: entry.code,
        word: entry.word,
        weight: entry.weight,
      });
  }

  async function reload(current: number, message: string) {
    if (!confirmed) return;
    try {
      await load(current, 0, confirmed);
      if (isCurrent(current)) setNotice(message);
    } catch {
      if (isCurrent(current)) setNotice(`${message}，但目录刷新失败，请重新查询`);
    }
  }

  function save() {
    if (!form) return;
    if (!form.code.trim() || !form.word.trim()) {
      setNotice("编码和词条不能为空");
      return;
    }
    return run(async (current) => {
      await client.request({
        operation: "edit_catalog",
        kind,
        code: form.entry.code,
        word: form.entry.word,
        revision,
        replacement: form.replacement
          ? { code: form.code.trim(), word: form.word, weight: form.weight }
          : null,
      });
      if (!isCurrent(current)) return;
      setForm(null);
      await reload(current, "完整目录已保存");
    }, "目录保存失败，请刷新后重试");
  }

  function remove(entry: CloudDictionaryCatalogEntry) {
    return run(async (current) => {
      await client.request({
        operation: "edit_catalog",
        kind,
        code: entry.code,
        word: entry.word,
        revision,
        replacement: null,
      });
      if (!isCurrent(current)) return;
      setForm((currentForm) =>
        currentForm?.entry.code === entry.code && currentForm.entry.word === entry.word
          ? null
          : currentForm,
      );
      await reload(current, "目录词条已删除");
    }, "目录删除失败，请刷新后重试");
  }

  async function confirmRemove(entry: CloudDictionaryCatalogEntry) {
    if (busyRef.current) return;
    const current = revisionRef.current;
    const confirmed = await confirm({
      title: "删除目录词条",
      message: `完整目录词条“${entry.word}”只会从云端删除，本机词库不会改变。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed || !isCurrent(current) || busyRef.current) return;
    void remove(entry);
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setEntries([]);
    setConfirmed(null);
    setForm(null);
    setOffset(0);
    setHasMore(false);
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
    code,
    setCode,
    scheme,
    setScheme,
    profile,
    setProfile,
    confirmed,
    entries,
    offset,
    hasMore,
    revision,
    normalized,
    form,
    notice,
    busy,
    queryCatalog,
    beginEdit,
    save,
    confirmRemove,
    changeKind,
    changeForm,
    cancelForm,
  };
}

export type CloudDictionaryCatalogPanelState = ReturnType<typeof useCloudDictionaryCatalogPanel>;
