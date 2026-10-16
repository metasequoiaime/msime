import {
  useCommunityResourceGallery,
  type CommunityResourceGalleryOptions,
} from "./use-community-resource-gallery";
import { CommunityResourceDetail } from "./community-resource-detail";
import { CommunityResourceEditor } from "./community-resource-editor";
import { CommunitySearchForm } from "./community-search-form";
import { resourceKindTitle } from "./community-helpers";
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
  type CommunityResourceScope,
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
