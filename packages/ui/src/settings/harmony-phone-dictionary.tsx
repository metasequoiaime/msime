import { AddDictionarySheet } from "../dictionary/add-dictionary-sheet";
import { DiscoverDictionaries } from "../dictionary/dictionary-discovery";
import { BuiltinDictionaryDetail, CollectionDetail } from "../dictionary/dictionary-details";
import { Badge, PlusGlyph } from "../dictionary/dictionary-detail-parts";
import { exceptionMessage } from "../core/exception-message";
import { downloadBlob } from "../core/download-blob";
import { useCallback, useEffect, useState } from "react";
import { formatZhNumber } from "../core/format-number";
import { GroupList, NavRow } from "../core/platform-controls";
import { useToast } from "../core/toast";
import { useAsyncActionRunner } from "../core/use-async-action";
import {
  binaryCollectionFormat,
  collectionFailureMessage,
  collectionFormatForFile,
  collectionImportMessage,
  collectionImportSources,
  collectionNameFromFile,
  collectionSubtitle,
  type DictionaryCollectionsView,
} from "../dictionary/dictionary-collections";
import { DictionaryNameDialog } from "../dictionary/dictionary-edit-dialogs";
import {
  dictionaryExportName,
  dictionaryExportPayload,
  loadDictionaryExportText,
} from "../dictionary/dictionary-export";
import { SCEL_PERSONAL_USE_NOTICE } from "../dictionary/dictionary-messages";
import {
  MAX_SCEL_FILE_BYTES,
  readDictionaryFile,
  readScelFile,
} from "../dictionary/dictionary-file";
import type { DictionaryClient } from "../dictionary/dictionary-contracts";
import { useMobileSettingsSubview } from "./use-mobile-settings-subview";
import { useSettingsForm } from "./settings-form-context";
import { useAsyncGeneration } from "./use-async-generation";
import { SwitchRow } from "./switch-row";

/** 命名词库一次导入的文本上限，与 client-services 的 `MAX_IMPORT_TEXT_BYTES` 相同。 */
const MAX_COLLECTION_IMPORT_BYTES = 16 * 1024 * 1024;
const BUILTIN = "builtin:pinyin";

/**
 * HarmonyOS 手机的「词库」页，按 Android 的 `LexiconPage` 布局：「已安装」列出拼音词库和用户自己的词库，点进去启用、停用和编辑；「管理」里新建、导入、导出和刷新；「发现词库」是社区词库，「添加」后成为一个可以停用或删除的词库，末尾的「浏览更多词库」切到底部「社区」标签的「词库」分类；然后是学习开关和「更多」。
 *
 * 词库详情在本页内打开，推入一条历史记录，系统返回键和左上角的返回都回到列表。
 */
export function HarmonyPhoneDictionary() {
  const {
    client,
    draft,
    setDraft,
    openPanel,
    pageEntry,
    selectPage,
    communityGallery,
    openCommunityGallery,
  } = useSettingsForm();
  const collections = client.dictionaryCollections;
  const dictionary = client.dictionary;
  const toast = useToast();
  const [view, setView] = useState<DictionaryCollectionsView | null>(null);
  const [loadFailure, setLoadFailure] = useState("");
  const [builtinCount, setBuiltinCount] = useState<number | null>(null);
  const {
    value: detail,
    open: openDictionaryDetail,
    close: closeDictionaryDetail,
  } = useMobileSettingsSubview<string>({
    mobile: true,
    stateKey: "dictionaryDetail",
    decode: (state, currentDetail) =>
      typeof state?.dictionaryDetail === "string" &&
      (currentDetail === null || state.dictionaryDetail === currentDetail)
        ? state.dictionaryDetail
        : null,
    encode: (id) => ({ page: "dictionary", dictionaryDetail: id }),
    isActiveState: (state) => Boolean(state?.dictionaryDetail),
  });
  const [creating, setCreating] = useState(false);
  const [addingDictionary, setAddingDictionary] = useState(false);
  const reportFailure = useCallback(
    (message: string) => {
      if (message) toast(message);
    },
    [toast],
  );
  const runner = useAsyncActionRunner(
    reportFailure,
    undefined,
    collections,
    dictionary,
    client.communityResources,
  );
  const { busy, mounted } = runner;
  const galleryOpen = communityGallery?.category === "dictionary";
  const reloadGeneration = useAsyncGeneration(collections, dictionary, galleryOpen);

  const reload = async (flush: boolean) => {
    const current = ++reloadGeneration.current;
    if (dictionary?.count) {
      dictionary
        .count("pinyin")
        .then(
          (count) =>
            mounted.current && current === reloadGeneration.current && setBuiltinCount(count),
        )
        .catch(() => undefined);
    }
    if (!collections) return;
    try {
      // 读列表时顺手把还没送出的增删再送一批，与 Android 打开词库页时相同。
      const next = flush ? await collections.flush() : await collections.load();
      if (!mounted.current || current !== reloadGeneration.current) return;
      setView(next);
      setLoadFailure("");
    } catch (error) {
      if (mounted.current && current === reloadGeneration.current)
        setLoadFailure(collectionFailureMessage(error));
    }
  };
  // 社区词库图库盖在这页上面，在里面添加的词库关掉图库时才读进来，「已安装」和「发现词库」的「已添加」随之更新。
  useEffect(() => {
    if (galleryOpen) return;
    void reload(true);
  }, [collections, dictionary, galleryOpen]);

  const run = async (
    action: (isCurrent: () => boolean) => Promise<DictionaryCollectionsView | undefined>,
    done?: string | (() => string),
  ) => {
    const current = runner.generation.current;
    let next: DictionaryCollectionsView | undefined;
    await runner.run(
      async (isCurrent) => {
        // 修改取得下一份列表的所有权，较早的刷新不能在回包时覆盖它。
        reloadGeneration.current++;
        const result = await action(isCurrent);
        if (!result || !isCurrent()) return;
        next = result;
        setView(result);
        if (done) toast(typeof done === "function" ? done() : done);
      },
      {
        formatError: collectionFailureMessage,
        onError: () => {
          if (collections) void reload(false);
        },
      },
    );
    return mounted.current && current === runner.generation.current ? next : undefined;
  };

  const openDetail = (id: string) => {
    openDictionaryDetail(id);
  };
  const closeDetail = () => {
    closeDictionaryDetail();
  };

  if (detail === BUILTIN && dictionary) {
    return (
      <BuiltinDictionaryDetail
        dictionary={dictionary}
        count={builtinCount}
        onExport={
          dictionary.export ? () => exportUserPinyin(dictionary, client.saveExport) : undefined
        }
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

  // 细胞词库按字节导入，宿主不能传字节时不列它。
  const importSources = collectionImportSources(
    (view?.formats ?? []).filter(
      (format) => !binaryCollectionFormat(format) || Boolean(collections?.importBytes),
    ),
  );
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
          onInstall={(item) => {
            let resource = item;
            return run(
              async (isCurrent) => {
                // 列表里的副本可能比服务器上的旧，词条取自重新读的详情，与社区页的「添加」相同。
                resource = await client.communityResources!.detail(item.id);
                if (!isCurrent()) return undefined;
                return collections.installCommunity(resource);
              },
              () => `已添加「${resource.name}」，可以在已安装里停用或删除`,
            );
          }}
          onBrowseMore={() =>
            openCommunityGallery({ category: "dictionary", scope: "", initialMine: false })
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
        <DictionaryNameDialog
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
            const current = runner.generation.current;
            const isCurrent = () => mounted.current && current === runner.generation.current;
            setAddingDictionary(false);
            const name = collectionNameFromFile(file.name);
            const format = collectionFormatForFile(file.name, source.format);
            const importBytes = collections.importBytes;
            if (binaryCollectionFormat(format)) {
              if (!importBytes) {
                toast(collectionFailureMessage({ code: "unsupported_format" }));
                return;
              }
              let encoded: string;
              try {
                encoded = await readScelFile(file, MAX_SCEL_FILE_BYTES);
              } catch (error) {
                if (isCurrent()) toast(exceptionMessage(error, "读取文件失败，请重新选择。"));
                return;
              }
              if (!isCurrent()) return;
              const next = await run(() => importBytes(name, format, encoded));
              if (next)
                toast(`${collectionImportMessage(name, next.import)}。${SCEL_PERSONAL_USE_NOTICE}`);
              return;
            }
            let text: string;
            try {
              text = await readDictionaryFile(file, MAX_COLLECTION_IMPORT_BYTES);
            } catch (error) {
              if (isCurrent()) toast(exceptionMessage(error, "读取文件失败，请重新选择。"));
              return;
            }
            if (!isCurrent()) return;
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

/** 「拼音词库」行的副标题。`count` 数的是自己加的词和学习时调过权重的内置词，不是内置词条的总量，所以照实写成「自己的词」；一条都没有时只写「内置词库」。 */
function builtinDescription(count: number | null): string {
  return count ? `内置词库 · ${formatZhNumber(count)} 条自己的词` : "内置词库";
}

/** 「导出词库」：分页读出自己加过的拼音词条，交给宿主的保存对话框；返回要提示的话，用户取消时返回空串。 */
async function exportUserPinyin(
  dictionary: DictionaryClient,
  saveExport: ((name: string, contents: string) => Promise<string | null>) | undefined,
): Promise<string> {
  if (!dictionary.export) return "";
  try {
    const text = await loadDictionaryExportText(dictionary, "pinyin", "standard");
    const payload = dictionaryExportPayload("pinyin", "standard", text);
    if (!payload.rows) return "还没有自己加过的拼音词条。";
    const name = dictionaryExportName("pinyin");
    if (saveExport) {
      const path = await saveExport(name, payload.body);
      return path === null ? "" : `已导出 ${formatZhNumber(payload.rows)} 条`;
    }
    downloadBlob(new Blob([payload.body], { type: "text/plain;charset=utf-8" }), name);
    return `已导出 ${formatZhNumber(payload.rows)} 条`;
  } catch (error) {
    return error instanceof Error && error.message && !error.message.startsWith("dictionary_")
      ? error.message
      : "词库导出失败，请稍后重试。";
  }
}
