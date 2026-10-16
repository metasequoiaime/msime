import { useState } from "react";
import {
  useCommunityResourceGallery,
  type CommunityResourceGalleryOptions,
} from "./use-community-resource-gallery";
import { CommunitySkinsPage } from "./community-skins";
import { CommunityResourceDetail } from "./community-resource-detail";
import { CommunityResourceEditor } from "./community-resource-editor";
import { CommunitySearchForm } from "./community-search-form";
import { type CommunitySkinClient } from "./community-skin-model";
import type { Preferences } from "../core/preferences-model";
import { resourceKindTitle } from "./community-helpers";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import * as style from "./community-style";
import { CommunityResourceCard } from "./community-resource-card";
import { CommunityHarmonyResourceRow } from "./community-harmony-resource-row";
import { CommunityGalleryHeader } from "./community-gallery-header";
import { CommunityResourceScopeButtons } from "./community-resource-scope-buttons";
import { CommunityNotice } from "./community-notice";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { ActionButton } from "../core/action-button";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import {
  type CommunityResourceKind,
  type CommunityResourceScope,
  type CommunityLocalDictionaryClient,
  type CommunityResourceClient,
} from "./community-resource-model";
export { addCommunityReply } from "./community-resource-actions";
export { communityResourceSubtitle } from "./community-resource-model";
export type {
  CommunityResourceKind,
  CommunityResourceScope,
  CommunitySharedWord,
  CommunityResourceContent,
  CommunityResource,
  CommunityResourcePage,
  CommunityResourceApplication,
  CommunityLocalDictionaryClient,
  CommunityResourceClient,
} from "./community-resource-model";

export interface CommunityResourcesPageProps extends CommunityResourceGalleryOptions {
  client: CommunityResourceClient;
  /** HarmonyOS 手机上的图库：胶囊搜索框、范围筛选 chip，以及一张由多行组成、每行带「添加」胶囊按钮的分组卡片。其他宿主不设置。 */
  look?: "harmony";
}

export function CommunityResourcesPage({
  client,
  kind,
  initialScope,
  localDictionary,
  mobile,
  look,
  historyPage,
}: CommunityResourcesPageProps) {
  const {
    scope,
    search,
    items,
    more,
    busy,
    error,
    selected,
    editing,
    added,
    adding,
    onScopeChange,
    onSearchChange,
    load,
    openDetail,
    closeDetail,
    add,
    openEditor,
    closeEditor,
    onPublished,
  } = useCommunityResourceGallery({
    client,
    kind,
    initialScope,
    localDictionary,
    mobile,
    historyPage,
  });
  if (selected)
    return (
      <CommunityResourceDetail
        client={client}
        initial={selected}
        onClose={closeDetail}
        localDictionary={localDictionary}
      />
    );
  if (look === "harmony") {
    const scopes: { scope: CommunityResourceScope; label: string }[] = [
      { scope: "", label: "全部" },
      { scope: "saved", label: "收藏" },
      { scope: "mine", label: "我的作品" },
    ];
    return (
      <div className={style.harmonyPage}>
        <CommunitySearchForm
          look="harmony"
          label={`搜索${resourceKindTitle(kind)}`}
          value={search}
          onChange={onSearchChange}
          onSubmit={() => void load(false, search)}
        />
        <div className={style.harmonyChips}>
          <div className="contents" role="group" aria-label={`${resourceKindTitle(kind)}范围`}>
            {scopes.map((option) => (
              <button
                key={option.scope || "all"}
                type="button"
                className={style.harmonyChip}
                aria-pressed={scope === option.scope}
                onClick={() => onScopeChange(option.scope)}
              >
                {option.label}
              </button>
            ))}
          </div>
          <span className={style.harmonyChipDivider} aria-hidden="true" />
          <button type="button" className={style.harmonyChipAction} onClick={openEditor}>
            发布作品
          </button>
        </div>
        <CommunityGalleryFeedback
          error={error}
          empty={
            !busy && items.length === 0 ? (
              <p className={style.harmonyNotice}>这里还没有{resourceKindTitle(kind)}作品。</p>
            ) : undefined
          }
        />
        {items.length > 0 && (
          <div className="flex min-w-0 flex-col">
            {kind === "reply" && <h3 className={style.harmonySectionTitle}>AI 回复模板</h3>}
            <div className={style.harmonyList}>
              {items.map((item) => (
                <CommunityHarmonyResourceRow
                  key={item.id}
                  item={item}
                  pill={adding.has(item.id) ? "adding" : added.has(item.id) ? "added" : "add"}
                  open={() => openDetail(item)}
                  add={() => void add(item)}
                />
              ))}
            </div>
          </div>
        )}
        <CommunityGalleryLoadMore
          look="harmony"
          hasMore={more}
          busy={busy}
          loadingText="正在读取社区…"
          onLoadMore={() => void load(true)}
        />
        {editing && (
          <CommunityResourceEditor
            client={client}
            kind={kind}
            onClose={closeEditor}
            onPublished={onPublished}
          />
        )}
      </div>
    );
  }
  return (
    <CommunityPageShell>
      <CommunityGalleryHeader
        searchLabel={`搜索${resourceKindTitle(kind)}`}
        search={search}
        onSearchChange={onSearchChange}
        onSearchSubmit={() => void load(false, search)}
        title={
          scope === "mine"
            ? `我的${resourceKindTitle(kind)}作品`
            : scope === "saved"
              ? `收藏的${resourceKindTitle(kind)}`
              : kind === "dictionary"
                ? "好词，随手可得"
                : "找到舒服的表达"
        }
        note={
          kind === "dictionary"
            ? "把常用词带进云词库，让输入更顺手"
            : "收藏喜欢的语气，给每次回应一点灵感"
        }
      >
        <CommunityResourceScopeButtons
          resourceLabel={resourceKindTitle(kind)}
          scope={scope}
          onScopeChange={onScopeChange}
        />
        <ActionButton action={openEditor} className="primary" label="发布作品" />
      </CommunityGalleryHeader>
      <CommunityGalleryFeedback
        error={error}
        empty={
          !busy && items.length === 0 ? (
            <CommunityNotice>这里还没有{resourceKindTitle(kind)}作品。</CommunityNotice>
          ) : undefined
        }
      />
      <CommunityGalleryGrid>
        {items.map((item) => (
          <CommunityResourceCard key={item.id} item={item} open={() => openDetail(item)} />
        ))}
      </CommunityGalleryGrid>
      <CommunityGalleryLoadMore
        hasMore={more}
        busy={busy}
        loadingText="正在读取社区…"
        onLoadMore={() => void load(true)}
      />
      {editing && (
        <CommunityResourceEditor
          client={client}
          kind={kind}
          onClose={closeEditor}
          onPublished={onPublished}
        />
      )}
    </CommunityPageShell>
  );
}

export function CommunityHomePage({
  skins,
  resources,
  theme,
  initialMine = false,
  initialCategory = "skin",
  initialScope = "",
  localDictionary,
  localSkinLibrary,
  mobile = false,
  onLogin,
  look,
  preferences,
  onApplyPreferences,
  onSkinApplied,
}: {
  /** 键盘皮肤画廊。Windows 没有（它的候选窗口皮肤社区在主题页），这时只有「词库」和「回复模板」两个分类。 */
  skins?: CommunitySkinClient;
  resources: CommunityResourceClient;
  theme: "light" | "dark";
  initialMine?: boolean;
  initialCategory?: "skin" | CommunityResourceKind;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  /**
   * 皮肤画廊用于发布的已保存设计。
   *
   * 这里原先缺了它，而同时有皮肤和资源的宿主渲染的正是这个页面，所以恰恰在这些宿主上（HarmonyOS 和 Android）「发布我的设计」从未绘制，发布流程根本没有入口。桌面端路径直接渲染 `CommunitySkinsPage`，没有问题。
   */
  localSkinLibrary?: CustomSkinLibraryClient;
  mobile?: boolean;
  onLogin?: () => void;
  /** HarmonyOS 手机的「社区」页：在重新设计的画廊上方放一个「皮肤 | 词库 | 短语」胶囊切换控件。其他宿主不设置。 */
  look?: "harmony";
  /** 转交给皮肤画廊，用于「使用」和「使用中」。 */
  preferences?: Preferences;
  onApplyPreferences?: (next: Preferences) => void | Promise<void>;
  onSkinApplied?: (id: string) => void;
}) {
  const [category, setCategory] = useState<"skin" | CommunityResourceKind>(
    skins || initialCategory !== "skin" ? initialCategory : "dictionary",
  );
  const harmony = look === "harmony";
  // 设计稿把第三段命名为「短语」。本宿主没有接入短语包，所以在 HarmonyOS 上这一段放的是回复模板，使用它们自己的分区标题。
  const tabs: { category: "skin" | CommunityResourceKind; label: string }[] = [
    ...(skins ? [{ category: "skin" as const, label: "皮肤" }] : []),
    { category: "dictionary", label: "词库" },
    { category: "reply", label: harmony ? "短语" : "回复模板" },
  ];
  const body = (
    <>
      <div
        className={harmony ? style.harmonySegments : style.categoryTabs}
        role="tablist"
        aria-label="社区分类"
      >
        {tabs.map((tab) => (
          <button
            key={tab.category}
            type="button"
            role="tab"
            aria-selected={category === tab.category}
            onClick={() => setCategory(tab.category)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      {category === "skin" && skins ? (
        <CommunitySkinsPage
          client={skins}
          theme={theme}
          initialMine={initialMine}
          localSkinLibrary={localSkinLibrary}
          mobile={mobile}
          onLogin={onLogin}
          look={look}
          preferences={preferences}
          onApplyPreferences={onApplyPreferences}
          onSkinApplied={onSkinApplied}
        />
      ) : (
        <CommunityResourcesPage
          client={resources}
          kind={category === "skin" ? "dictionary" : category}
          initialScope={initialScope}
          localDictionary={localDictionary}
          mobile={mobile}
          look={look}
        />
      )}
    </>
  );
  return harmony ? (
    <div className={style.harmonyPage}>{body}</div>
  ) : (
    <CommunityPageShell>{body}</CommunityPageShell>
  );
}
