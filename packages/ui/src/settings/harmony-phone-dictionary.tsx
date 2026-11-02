import { AddDictionarySheet } from "../dictionary/add-dictionary-sheet";
import { DiscoverDictionaries } from "../dictionary/dictionary-discovery";
import { BuiltinDictionaryDetail, CollectionDetail } from "../dictionary/dictionary-details";
import { Badge, PlusGlyph } from "../dictionary/dictionary-detail-parts";
import { useState } from "react";
import { formatZhNumber } from "../core/format-number";
import { GroupList, NavRow } from "../core/platform-controls";
import { SettingsGroupNote } from "./settings-group-note";
import { useToast } from "../core/toast";
import {
  binaryCollectionFormat,
  collectionImportSources,
  collectionSubtitle,
} from "../dictionary/dictionary-collections";
import { DictionaryNameDialog } from "../dictionary/dictionary-edit-dialogs";
import { exportPersonalPinyinDictionary } from "../dictionary/dictionary-export-action";
import { importDictionaryCollectionFile } from "../dictionary/dictionary-collection-import-action";
import { useMobileSettingsSubview } from "./use-mobile-settings-subview";
import { useSettingsForm } from "./settings-form-context";
import { useDictionaryCollectionsState } from "../dictionary/use-dictionary-collections-state";
import { SwitchRow } from "./switch-row";

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
  const { view, loadFailure, builtinCount, busy, run, captureOwner } =
    useDictionaryCollectionsState({
      collections,
      dictionary,
      community: client.communityResources,
      galleryOpen: communityGallery?.category === "dictionary",
      onNotice: toast,
    });

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
          dictionary.export
            ? () => exportPersonalPinyinDictionary(dictionary, client.saveExport)
            : undefined
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
          <SettingsGroupNote>
            点进词库可以启用、停用和编辑词条。已启用的词库会一起参与候选。
          </SettingsGroupNote>
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
        {/* 怎么用是解释，收进「已安装」组名旁的「?」；读取失败是错误，照旧画在原处。 */}
        {loadFailure && (
          <p className={footnote} role="alert">
            {loadFailure}
          </p>
        )}
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
            const isCurrent = captureOwner();
            setAddingDictionary(false);
            const message = await importDictionaryCollectionFile({
              collections,
              source,
              file,
              isCurrent,
              run,
            });
            if (message && isCurrent()) toast(message);
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
