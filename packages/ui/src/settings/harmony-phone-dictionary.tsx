import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { ActionSheet } from "../core/action-sheet";
import { useConfirm } from "../core/confirm";
import { formatZhNumber } from "../core/format-number";
import { GroupList, NavRow } from "../core/platform-controls";
import * as controls from "../core/platform-controls-style";
import { useToast } from "../core/toast";
import * as dialog from "../community/community-style";
import type { CommunityResource, CommunityResourceClient } from "../community/community-resources";
import { communityResourceSubtitle } from "../community/community-resources";
import {
  collectionFailureMessage,
  collectionFormatForFile,
  collectionImportMessage,
  collectionImportSources,
  collectionNameFromFile,
  collectionSubtitle,
  displayPinyinCode,
  normalizePinyinCode,
  validCollectionName,
  validPinyinCode,
  type CollectionImportSource,
  type DictionaryCollection,
  type DictionaryCollectionsView,
} from "../dictionary/dictionary-collections";
import { dictionaryExportName, dictionaryExportPayload } from "../dictionary/dictionary-export";
import { readDictionaryFile, type DictionaryEntry } from "../dictionary/dictionary-file";
import type { DictionaryClient } from "../index";
import { pushMobileSettingsState } from "./mobile-navigation";
import { useMobilePopState } from "./use-mobile-pop-state";
import { useSettingsForm } from "./settings-form-context";
import { SwitchRow } from "./switch-row";
import * as settings from "./settings-style";

/** 「发现词库」列出的社区词库数，与 Android 的 `DISCOVER_LIMIT` 相同。 */
const DISCOVER_LIMIT = 8;
/** 内置词库详情页一次读的词条数，与 Android 的词库详情页相同。 */
const ENTRY_PAGE_SIZE = 100;
/** 命名词库一次导入的文本上限，与 client-core 的 `MAX_IMPORT_TEXT_BYTES` 相同。 */
const MAX_COLLECTION_IMPORT_BYTES = 16 * 1024 * 1024;
const BUILTIN = "builtin:pinyin";

/**
 * HarmonyOS 手机的「词库」页，按 Android 的 `LexiconPage` 布局：「已安装」列出拼音词库和用户自己的词库，点进去启用、停用和编辑；「管理」里新建、导入、导出和刷新；「发现词库」是社区词库，「添加」后成为一个可以停用或删除的词库；然后是学习开关和「更多」。
 *
 * 词库详情在本页内打开，推入一条历史记录，系统返回键和左上角的返回都回到列表。
 */
export function HarmonyPhoneDictionary() {
  const { client, draft, setDraft, openPanel, pageEntry, selectPage } = useSettingsForm();
  const collections = client.dictionaryCollections;
  const dictionary = client.dictionary;
  const toast = useToast();
  const [view, setView] = useState<DictionaryCollectionsView | null>(null);
  const [loadFailure, setLoadFailure] = useState("");
  const [builtinCount, setBuiltinCount] = useState<number | null>(null);
  const [detail, setDetail] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [addingDictionary, setAddingDictionary] = useState(false);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(true);
  useEffect(
    () => () => {
      mounted.current = false;
    },
    [],
  );

  const reload = async (flush: boolean) => {
    if (dictionary?.count) {
      dictionary
        .count("pinyin")
        .then((count) => mounted.current && setBuiltinCount(count))
        .catch(() => undefined);
    }
    if (!collections) return;
    try {
      // 读列表时顺手把还没送出的增删再送一批，与 Android 打开词库页时相同。
      const next = flush ? await collections.flush() : await collections.load();
      if (!mounted.current) return;
      setView(next);
      setLoadFailure("");
    } catch (error) {
      if (mounted.current) setLoadFailure(collectionFailureMessage(error));
    }
  };
  useEffect(() => {
    void reload(true);
  }, [collections, dictionary]);

  const run = async (action: () => Promise<DictionaryCollectionsView>, done?: string) => {
    if (busy) return undefined;
    setBusy(true);
    try {
      const next = await action();
      if (!mounted.current) return undefined;
      setView(next);
      if (done) toast(done);
      return next;
    } catch (error) {
      if (mounted.current) toast(collectionFailureMessage(error));
      if (mounted.current && collections) void reload(false);
      return undefined;
    } finally {
      if (mounted.current) setBusy(false);
    }
  };

  const openDetail = (id: string) => {
    if (typeof window !== "undefined") {
      pushMobileSettingsState({ page: "dictionary", dictionaryDetail: id });
    }
    setDetail(id);
  };
  const closeDetail = () => {
    if (typeof window !== "undefined" && window.history.state?.dictionaryDetail) {
      window.history.back();
      return;
    }
    setDetail(null);
  };
  useMobilePopState(true, (event) => {
    if (detail && event.state?.dictionaryDetail !== detail) setDetail(null);
  });

  if (detail === BUILTIN && dictionary) {
    return (
      <BuiltinDictionaryDetail
        dictionary={dictionary}
        count={builtinCount}
        saveExport={client.saveExport}
      />
    );
  }
  const opened = detail ? view?.collections.find((item) => item.id === detail) : undefined;
  if (detail && opened && collections) {
    return (
      <CollectionDetail
        collection={opened}
        busy={busy}
        onEnabled={(enabled) =>
          void run(() => collections.setEnabled(opened.id, enabled), enabled ? "已启用" : "已停用")
        }
        onAddWord={(word, code) =>
          run(
            () =>
              collections.addWords(opened.id, [
                { kind: "pinyin", key: code, value: word, weight: 10 },
              ]),
            "已添加",
          )
        }
        onDelete={async () => {
          const next = await run(() => collections.delete(opened.id), `已删除「${opened.name}」`);
          if (next) closeDetail();
        }}
      />
    );
  }

  const importSources = collectionImportSources(view?.formats ?? []);
  const learning = draft?.learning ?? true;
  const vocabulary = pageEntry("vocabulary");
  return (
    <>
      <div className="flex flex-col gap-2">
        <GroupList title="已安装">
          {dictionary && (
            <NavRow
              variant="me"
              icon={<Badge>汉</Badge>}
              title="拼音词库"
              description={builtinDescription(builtinCount)}
              value="已启用"
              onClick={() => openDetail(BUILTIN)}
            />
          )}
          {view?.collections.map((item) => (
            <NavRow
              key={item.id}
              variant="me"
              icon={<Badge>{Array.from(item.name)[0] ?? "词"}</Badge>}
              title={item.name}
              description={collectionSubtitle(item)}
              value={item.enabled ? "已启用" : "已停用"}
              onClick={() => openDetail(item.id)}
            />
          ))}
          {collections && (
            <NavRow
              variant="me"
              icon={
                <Badge>
                  <PlusGlyph />
                </Badge>
              }
              title="添加词库"
              description="新建一个空词库，或从文件导入"
              disabled={busy}
              onClick={() => setAddingDictionary(true)}
            />
          )}
        </GroupList>
        <p className={footnote} role={loadFailure ? "alert" : undefined}>
          {loadFailure || "点进词库可以启用、停用和编辑词条。已启用的词库会一起参与候选。"}
        </p>
      </div>
      {collections && client.communityResources && (
        <DiscoverDictionaries
          community={client.communityResources}
          view={view}
          busy={busy}
          onInstall={(resource) =>
            run(
              () => collections.installCommunity(resource),
              `已添加「${resource.name}」，可以在已安装里停用或删除`,
            )
          }
        />
      )}
      {draft && (
        <GroupList title="学习">
          <SwitchRow
            title="记忆新词"
            description="把你选过的词排到前面；只在本机学习"
            checked={learning}
            onChange={(next) =>
              setDraft((current) => (current ? { ...current, learning: next } : current))
            }
          />
        </GroupList>
      )}
      {(vocabulary || client.openCloudDictionary) && (
        <GroupList title="更多">
          {vocabulary && (
            <NavRow
              variant="me"
              title={vocabulary.title}
              description="用输入过的英文单词复习词汇"
              onClick={() => selectPage("vocabulary")}
            />
          )}
          {client.openCloudDictionary && (
            <NavRow
              variant="me"
              title="云词库"
              description="管理同步到账号的词条和备份"
              onClick={() => openPanel(client.openCloudDictionary)}
            />
          )}
        </GroupList>
      )}
      {creating && collections && (
        <NameDialog
          onClose={() => setCreating(false)}
          onSubmit={async (name) => {
            const before = new Set(view?.collections.map((item) => item.id));
            const next = await run(() => collections.create(name), `已新建「${name}」`);
            if (!next) return;
            setCreating(false);
            const created = next.collections.find((item) => !before.has(item.id));
            if (created) openDetail(created.id);
          }}
        />
      )}
      {collections && (
        <AddDictionarySheet
          open={addingDictionary}
          sources={importSources}
          onClose={() => setAddingDictionary(false)}
          onCreate={() => {
            setAddingDictionary(false);
            setCreating(true);
          }}
          onFile={async (source, file) => {
            setAddingDictionary(false);
            let text: string;
            try {
              text = await readDictionaryFile(file, MAX_COLLECTION_IMPORT_BYTES);
            } catch (error) {
              toast(error instanceof Error ? error.message : "读取文件失败，请重新选择。");
              return;
            }
            const name = collectionNameFromFile(file.name);
            const format = collectionFormatForFile(file.name, source.format);
            const next = await run(() => collections.importFile(name, format, text));
            if (next) toast(collectionImportMessage(name, next.import));
          }}
        />
      )}
    </>
  );
}

/** 卡片下方的小字说明，与 Android 词库页「已安装」卡片的脚注一样放在卡片外面。 */
const footnote = "m-0 px-4 text-[13px] leading-relaxed [color:var(--p-sub)]";

/** 删除这类危险操作：一行红字，没有图标和箭头。 */
function DangerRow({
  title,
  disabled,
  onClick,
}: {
  title: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`${controls.navRow} ${controls.navRowButton}`}
      disabled={disabled}
      onClick={onClick}
    >
      {/* 颜色给到文字上：行本身的样式固定用正文色。 */}
      <span
        className="flex min-h-[52px] min-w-0 flex-1 items-center text-[16px] text-danger"
        data-row-title=""
      >
        {title}
      </span>
    </button>
  );
}

/** 「添加」的加号，画在和词库行同样的字标方块里，看上去就是往列表里再加一项。图标库没有加号，这里直接画两条居中的线。 */
function PlusGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <path d="M8 2.5v11M2.5 8h11" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

/** 「拼音词库」行的副标题。`count` 数的是自己加的词和学习时调过权重的内置词，不是内置词条的总量，所以照实写成「自己的词」；一条都没有时只写「内置词库」。 */
function builtinDescription(count: number | null): string {
  return count ? `内置词库 · ${formatZhNumber(count)} 条自己的词` : "内置词库";
}

/** 行首的字标：词库名的第一个字，或者操作的符号，和 Android 词库页的徽标一样。 */
function Badge({ children }: { children: ReactNode }) {
  return (
    <span className="flex size-7 items-center justify-center rounded-[8px] text-[14px] font-semibold [background:color-mix(in_srgb,var(--p-accent-text)_14%,transparent)] [color:var(--p-accent-text)]">
      {children}
    </span>
  );
}

/** 「导出词库」：分页读出自己加过的拼音词条，交给宿主的保存对话框；返回要提示的话，用户取消时返回空串。 */
async function exportUserPinyin(
  dictionary: DictionaryClient,
  saveExport: ((name: string, contents: string) => Promise<string | null>) | undefined,
): Promise<string> {
  if (!dictionary.export) return "";
  try {
    let text = "";
    let offset = 0;
    let more = true;
    while (more && offset <= 1_000_000) {
      const page = await dictionary.export("pinyin", "standard", offset, 1000);
      text += page.text;
      const count = page.text ? page.text.trimEnd().split("\n").length : 0;
      offset += count;
      more = page.has_more && count > 0;
    }
    const payload = dictionaryExportPayload("pinyin", "standard", text);
    if (!payload.rows) return "还没有自己加过的拼音词条。";
    const name = dictionaryExportName("pinyin");
    if (saveExport) {
      const path = await saveExport(name, payload.body);
      return path === null ? "" : `已导出 ${formatZhNumber(payload.rows)} 条`;
    }
    const url = URL.createObjectURL(new Blob([payload.body], { type: "text/plain;charset=utf-8" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    URL.revokeObjectURL(url);
    return `已导出 ${formatZhNumber(payload.rows)} 条`;
  } catch (error) {
    return error instanceof Error && error.message && !error.message.startsWith("dictionary_")
      ? error.message
      : "词库导出失败，请稍后重试。";
  }
}

/** 「添加词库」：底部面板里先是「新建空词库」，再是各种导入来源；选了来源就打开系统的文件选择器，导入的词条成为一个新词库。 */
function AddDictionarySheet({
  open,
  sources,
  onClose,
  onCreate,
  onFile,
}: {
  open: boolean;
  sources: CollectionImportSource[];
  onClose: () => void;
  onCreate: () => void;
  onFile: (source: CollectionImportSource, file: File) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const chosen = useRef<CollectionImportSource | null>(null);
  return (
    <>
      <ActionSheet
        open={open}
        title="添加词库"
        subtitle="导入的词条会成为一个新的词库"
        options={[
          { value: "create", label: "新建空词库" },
          ...sources.map((source) => ({ value: source.format, label: source.title })),
        ]}
        onSelect={(value) => {
          if (value === "create") {
            onCreate();
            return;
          }
          const source = sources.find((item) => item.format === value);
          if (!source || !input.current) return;
          chosen.current = source;
          input.current.accept = source.accept;
          input.current.click();
        }}
        onClose={onClose}
      />
      <input
        ref={input}
        hidden
        type="file"
        aria-label="选择词库文件"
        onChange={(event) => {
          const file = event.target.files?.[0];
          const source = chosen.current;
          event.currentTarget.value = "";
          if (file && source) onFile(source, file);
          else onClose();
        }}
      />
    </>
  );
}

/** 「发现词库」：社区词库的前几个，「添加」后成为一个独立的词库；已经装过的显示「已添加」。 */
function DiscoverDictionaries({
  community,
  view,
  busy,
  onInstall,
}: {
  community: CommunityResourceClient;
  view: DictionaryCollectionsView | null;
  busy: boolean;
  onInstall: (resource: CommunityResource) => Promise<DictionaryCollectionsView | undefined>;
}) {
  const [items, setItems] = useState<CommunityResource[] | null>(null);
  const [failure, setFailure] = useState("");
  const [adding, setAdding] = useState<string | null>(null);
  useEffect(() => {
    let current = true;
    community
      .list("dictionary", "", "", 0)
      .then((page) => current && setItems(page.items.slice(0, DISCOVER_LIMIT)))
      .catch(() => current && setFailure("社区词库暂时读不到，请稍后再试。"));
    return () => {
      current = false;
    };
  }, [community]);
  const installed = (id: string) =>
    view?.collections.some(
      (item) => item.source.type === "community" && item.source.resource_id === id,
    ) ?? false;
  return (
    <GroupList title="发现词库">
      {items === null || items.length === 0 ? (
        <p className={settings.groupNote} role={failure ? "alert" : undefined}>
          {failure || (items === null ? "正在读取社区词库…" : "社区里还没有词库。")}
        </p>
      ) : (
        items.map((item) => {
          const done = installed(item.id);
          const pending = adding === item.id;
          return (
            <NavRow
              key={item.id}
              variant="me"
              icon={<Badge>{Array.from(item.name)[0] ?? "词"}</Badge>}
              title={item.name}
              description={communityResourceSubtitle(item)}
              trailing={
                <button
                  type="button"
                  className="secondary"
                  aria-label={`添加词库 ${item.name}`}
                  disabled={done || pending || busy}
                  onClick={async () => {
                    setAdding(item.id);
                    try {
                      // 列表里的副本可能比服务器上的旧，词条取自重新读的详情，与社区页的「添加」相同。
                      await onInstall(await community.detail(item.id));
                    } finally {
                      setAdding(null);
                    }
                  }}
                >
                  {done ? "已添加" : pending ? "添加中" : "添加"}
                </button>
              }
            />
          );
        })
      )}
    </GroupList>
  );
}

/** 词库详情页的标题。返回不另放按钮：打开详情时推入了一条历史记录，页面顶栏的返回和系统返回键都先回到词库列表，与 Android 的词库详情页一样只有一个返回。 */
function DetailHeader({ title }: { title: string }) {
  return (
    <div className={settings.subViewHeader}>
      <h2 className={settings.subViewTitle}>{title}</h2>
    </div>
  );
}

/** 一个命名词库：启用开关、条数、添加词条和删除。集合接口不列出单个集合的词条，所以和 Android 一样只显示条数。 */
function CollectionDetail({
  collection,
  busy,
  onEnabled,
  onAddWord,
  onDelete,
}: {
  collection: DictionaryCollection;
  busy: boolean;
  onEnabled: (enabled: boolean) => void;
  onAddWord: (word: string, code: string) => Promise<DictionaryCollectionsView | undefined>;
  onDelete: () => Promise<void>;
}) {
  const [adding, setAdding] = useState(false);
  const { confirm, confirmation } = useConfirm();
  const pending =
    collection.pending > 0 ? `，还有 ${formatZhNumber(collection.pending)} 条等键盘同步` : "";
  return (
    <>
      <DetailHeader title={collection.name} />
      <GroupList>
        <SwitchRow
          title="启用此词库"
          description={`${formatZhNumber(collection.entry_count)} 条${pending}`}
          checked={collection.enabled}
          disabled={busy}
          onChange={onEnabled}
        />
      </GroupList>
      <GroupList>
        <NavRow
          variant="me"
          icon={
            <Badge>
              <PlusGlyph />
            </Badge>
          }
          title="添加词条"
          disabled={busy}
          onClick={() => setAdding(true)}
        />
      </GroupList>
      <GroupList>
        <DangerRow
          title="删除此词库"
          disabled={busy}
          onClick={async () => {
            const confirmed = await confirm({
              title: `删除「${collection.name}」？`,
              message: "这个词库带来的词会从候选里去掉，你自己加过的词会留下。",
              confirmLabel: "删除",
              danger: true,
            });
            if (confirmed) await onDelete();
          }}
        />
      </GroupList>
      {adding && (
        <WordDialog
          onClose={() => setAdding(false)}
          onSubmit={async (word, code) => {
            if (await onAddWord(word, code)) setAdding(false);
          }}
        />
      )}
      {confirmation}
    </>
  );
}

/** 内置拼音词库：始终启用，可以搜词条、加词和导出自己加过的词。 */
function BuiltinDictionaryDetail({
  dictionary,
  count,
  saveExport,
}: {
  dictionary: DictionaryClient;
  count: number | null;
  saveExport: ((name: string, contents: string) => Promise<string | null>) | undefined;
}) {
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<DictionaryEntry[]>([]);
  const [more, setMore] = useState(false);
  const [loading, setLoading] = useState(true);
  const [adding, setAdding] = useState(false);
  const generation = useRef(0);
  const searchId = useId();

  const load = async (search: string, offset: number) => {
    const current = ++generation.current;
    setLoading(true);
    const code = normalizePinyinCode(search);
    const byCode = search.trim() !== "" && validPinyinCode(code);
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
        search.trim() !== "" && !byCode
          ? page.entries.filter((entry) => entry.value.includes(search.trim()))
          : page.entries;
      setEntries((value) => (offset === 0 ? found : [...value, ...found]));
      setMore(page.has_more);
    } catch {
      if (current === generation.current) toast("读取词条失败，请稍后重试。");
    } finally {
      if (current === generation.current) setLoading(false);
    }
  };
  useEffect(() => {
    const timer = setTimeout(() => void load(query, 0), query ? 250 : 0);
    return () => clearTimeout(timer);
  }, [query, dictionary]);

  return (
    <>
      <DetailHeader title="拼音词库" />
      <GroupList>
        <SwitchRow
          title="启用此词库"
          description={`内置词库始终启用${count ? `，有 ${formatZhNumber(count)} 条自己的词` : ""}`}
          checked
          disabled
          onChange={() => undefined}
        />
      </GroupList>
      <label htmlFor={searchId} className="sr-only">
        搜索词条
      </label>
      <input
        id={searchId}
        type="search"
        className="box-border h-10 w-full rounded-full border-0 px-4 text-[15px] [background:var(--p-group-bg)] [color:var(--p-text)]"
        placeholder="搜索词条"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      {/* 与 Android 的词库详情页一样：词条和「添加词条」在同一张卡片里，导出单独一张。 */}
      <GroupList>
        {entries.map((entry) => (
          <div
            key={`${entry.key}\u0000${entry.value}`}
            className="flex min-h-[48px] items-center gap-3 px-3.5 py-2"
          >
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[16px] [color:var(--p-text)]">
                {entry.value}
              </span>
              <span className="block truncate text-[12px] [color:var(--p-sub)]">
                {displayPinyinCode(entry.key)}
              </span>
            </span>
            <span className="shrink-0 text-[13px] [color:var(--p-sub)]">{entry.weight}</span>
          </div>
        ))}
        {!loading && entries.length === 0 && (
          <p className={settings.groupNote}>
            {query ? "没有匹配的词条。" : "还没有自己添加或学到的词。输入拼音可以搜索内置词条。"}
          </p>
        )}
        {more && (
          <NavRow
            variant="me"
            title={loading ? "正在读取…" : "加载更多"}
            disabled={loading}
            onClick={() => void load(query, entries.length)}
          />
        )}
        <NavRow
          variant="me"
          icon={
            <Badge>
              <PlusGlyph />
            </Badge>
          }
          title="添加词条"
          onClick={() => setAdding(true)}
        />
      </GroupList>
      {dictionary.export && (
        <GroupList>
          <NavRow
            variant="me"
            title="导出词库"
            description="导出自己加过的拼音词条"
            onClick={() =>
              void exportUserPinyin(dictionary, saveExport).then(
                (message) => message && toast(message),
              )
            }
          />
        </GroupList>
      )}
      {adding && (
        <WordDialog
          onClose={() => setAdding(false)}
          onSubmit={async (word, code) => {
            try {
              await dictionary.edit(
                null,
                { kind: "pinyin", key: code, value: word, weight: 10 },
                `phone-dictionary-${Date.now()}`,
              );
              setAdding(false);
              toast("已添加");
              void load(query, 0);
            } catch {
              toast("添加失败，请稍后重试。");
            }
          }}
        />
      )}
    </>
  );
}

/** 页内的小对话框：标题、若干输入框、取消和确认。 */
function FormDialog({
  title,
  submitLabel,
  error,
  onClose,
  onSubmit,
  children,
}: {
  title: string;
  submitLabel: string;
  error: string;
  onClose: () => void;
  onSubmit: () => Promise<void>;
  children: ReactNode;
}) {
  const titleId = useId();
  const [submitting, setSubmitting] = useState(false);
  const submit = async () => {
    if (submitting) return;
    setSubmitting(true);
    try {
      await onSubmit();
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <div
      // 与底部动作面板同一层：手机上收起的页头和底部标签栏都在 z-20 之上，键盘弹起时会把对话框的按钮盖住。
      className={`${dialog.backdrop} z-50`}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !submitting) onClose();
      }}
    >
      {/* 不用 <form>：整个设置页已经在一个 <form> 里，嵌套的表单在 WebView 里会被当成外层表单提交，页面整个重新加载回首页。回车由输入框自己确认。 */}
      <div
        className={`${dialog.dialog} w-[min(420px,100%)]`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !submitting) onClose();
          if (event.key === "Enter" && event.target instanceof HTMLInputElement) {
            event.preventDefault();
            void submit();
          }
        }}
      >
        <h2 id={titleId} className={dialog.dialogTitle}>
          {title}
        </h2>
        {children}
        {error && (
          <p role="alert" className="m-0 text-[13px] text-danger">
            {error}
          </p>
        )}
        <div className={dialog.dialogActions}>
          <button type="button" className="secondary" disabled={submitting} onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="primary"
            disabled={submitting}
            onClick={() => void submit()}
          >
            {submitLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

function DialogField({
  label,
  value,
  placeholder,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex flex-col gap-1.5 text-[14px] [color:var(--p-sub)]">
      {label}
      <input
        className={dialog.fieldControl}
        aria-label={label}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
      />
    </label>
  );
}

/** 「新建词库」：起个名字，比如「工作」「游戏」。 */
function NameDialog({
  onClose,
  onSubmit,
}: {
  onClose: () => void;
  onSubmit: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  return (
    <FormDialog
      title="新建词库"
      submitLabel="创建"
      error={error}
      onClose={onClose}
      onSubmit={async () => {
        const trimmed = name.trim();
        if (!validCollectionName(trimmed)) {
          setError("词库名需要 1–32 个字。");
          return;
        }
        await onSubmit(trimmed);
      }}
    >
      <DialogField
        label="词库名称"
        value={name}
        placeholder="比如「工作」「游戏」"
        onChange={(value) => {
          setName(value);
          setError("");
        }}
      />
    </FormDialog>
  );
}

/** 「添加词条」：词语和它的拼音。 */
function WordDialog({
  onClose,
  onSubmit,
}: {
  onClose: () => void;
  onSubmit: (word: string, code: string) => Promise<void>;
}) {
  const [word, setWord] = useState("");
  const [pinyin, setPinyin] = useState("");
  const [error, setError] = useState("");
  return (
    <FormDialog
      title="添加词条"
      submitLabel="添加"
      error={error}
      onClose={onClose}
      onSubmit={async () => {
        const value = word.trim();
        const code = normalizePinyinCode(pinyin);
        if (!value) {
          setError("请填写词语。");
          return;
        }
        if (!validPinyinCode(code)) {
          setError("拼音只能是字母，音节之间可以用空格或 ' 分开。");
          return;
        }
        await onSubmit(value, code);
      }}
    >
      <DialogField
        label="词语"
        value={word}
        onChange={(value) => {
          setWord(value);
          setError("");
        }}
      />
      <DialogField
        label="拼音"
        value={pinyin}
        placeholder="比如 shui shan"
        onChange={(value) => {
          setPinyin(value);
          setError("");
        }}
      />
    </FormDialog>
  );
}
