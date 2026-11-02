import type { SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { CandidateSkinPublishDialog } from "./candidate-skin-publish-dialog";
import {
  candidateSkinMessage,
  communityNeedsSignIn,
  communityPublishLoginAction,
} from "./community-helpers";
import { CommunityGalleryHeader } from "./community-gallery-header";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { CommunityNotice } from "./community-notice";
import { ActionButton } from "../core/action-button";
import { CommunitySkinCategoryFilter } from "./community-skin-category";
import { CommunityCandidateSkinCard } from "./community-candidate-skin-card";
import { CommunityCandidateSkinDetail } from "./community-candidate-skin-detail";

import type { CandidateSkinCommunityClient } from "./community-candidate-skin-model";
import { useCommunityCandidateSkinGallery } from "./use-community-candidate-skin-gallery";
export {
  type CommunityCandidateSkinLicense,
  type CandidateSkinVisibility,
  type CandidateSkinCategory,
  candidateSkinCategories,
  candidateSkinCategoryLabels,
  type CommunityCandidateSkin,
  type CommunityCandidateSkinPage,
  type CandidateSkinPackPreview,
  type CandidateSkinSyncSkip,
  type CandidateSkinSyncReport,
  type CandidateSkinCommunityClient,
} from "./community-candidate-skin-model";

/** 候选窗口皮肤图库的展示入口，业务状态与动作由公共图库 Hook 提供。 */
export function CommunityCandidateSkinsPage({
  client,
  localSkins,
  openSkinDirectory,
  readSkinImage,
  onOpenSkinPage,
  onLogin,
}: {
  client: CandidateSkinCommunityClient;
  /** The installed packages: the publish choices, and how an install learns it would replace one. */
  localSkins?: () => Promise<SkinCatalog>;
  openSkinDirectory?: () => Promise<void>;
  readSkinImage?: SkinImageReader;
  /** 回到主题页的本地皮肤标签启用已安装的皮肤，社区页面位于设置表单之外，不直接写偏好。 */
  onOpenSkinPage?: () => void;
  onLogin?: () => void;
}) {
  const {
    sync,
    skins,
    hasMore,
    listBusy,
    detailBusy,
    error,
    selected,
    actionBusy,
    actionNotice,
    mineOnly,
    signInRequired,
    confirmUnpublish,
    activeSearch,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    reportSelected,
    blocked,
    search,
    setSearch,
    publishOpen,
    setPublishOpen,
    installed,
    confirmReplace,
    setConfirmReplace,
    categoryFilter,
    changeCategory,
    loadPreview,
    install,
    changeVisibility,
    changeOwnCategory,
    unpublish,
    publishDone,
  } = useCommunityCandidateSkinGallery({ client, localSkins });

  if (selected) {
    return (
      <CommunityCandidateSkinDetail
        skin={selected}
        load={loadPreview}
        actionBusy={actionBusy}
        detailBusy={detailBusy}
        actionNotice={actionNotice}
        error={error}
        signInRequired={signInRequired}
        installed={installed}
        confirmReplace={confirmReplace}
        confirmUnpublish={confirmUnpublish}
        onLogin={onLogin}
        onBack={closeDetail}
        onInstall={(replace) => void install(replace)}
        onCancelReplace={() => setConfirmReplace(false)}
        onOpenSkinPage={onOpenSkinPage}
        onChangeVisibility={(visibility) => void changeVisibility(visibility)}
        onChangeCategory={(next) => void changeOwnCategory(next)}
        onRate={(stars) => void rateSelected(stars)}
        onRequestUnpublish={() => setConfirmUnpublish(true)}
        onUnpublish={() => void unpublish()}
        onCancelUnpublish={() => setConfirmUnpublish(false)}
        onReport={client.report ? reportSelected : undefined}
        onBlocked={(notice) => void blocked(notice)}
      />
    );
  }

  return (
    <CommunityPageShell>
      <CommunityGalleryHeader
        searchLabel="搜索候选窗口皮肤"
        search={search}
        onSearchChange={setSearch}
        onSearchSubmit={() => void requestList(search, false)}
        title="社区皮肤"
        help={
          <>
            <span className="block">为输入候选窗口换一身新装，安装后在「本地皮肤」中启用。</span>
            <span className="mt-1.5 block">
              MCP 可以让 Claude、Codex 等 AI 助手在本机帮你制作候选窗口皮肤。先到桌面设置的「AI →
              连接助手」（手机为「AI 助手」）连接
              MCP，打开「允许修改设置」，再告诉助手想要的颜色、风格和明暗模式；例如“帮我做一款深色的青绿色皮肤”。助手会生成并安装皮肤，登录后还会保存到你的皮肤库。回到这里点「发布皮肤」，就能把它分享给社区。
            </span>
          </>
        }
      />
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <CommunityScopeButtons
          className="flex gap-0.5 rounded-lg bg-subtle p-0.5 [&>button]:m-0 [&>button]:rounded-md [&>button]:border-0 [&>button]:bg-transparent [&>button]:px-3 [&>button]:py-1.5 [&>button]:font-medium [&>button]:whitespace-nowrap [&>button]:text-secondary [&>button[aria-pressed=true]]:bg-raised [&>button[aria-pressed=true]]:text-body [&>button[aria-pressed=true]]:shadow-card"
          ariaLabel="候选窗口皮肤范围"
          mineOnly={mineOnly}
          allLabel="全部"
          mineLabel="我的作品"
          onMineOnlyChange={changeMineOnly}
        />
        <div className="flex items-center gap-2 [&>button]:m-0 [&>button]:whitespace-nowrap">
          <button
            type="button"
            className="secondary"
            onClick={() => void sync.run()}
            disabled={sync.busy}
            aria-busy={sync.busy}
            aria-label={sync.busy ? "正在同步皮肤库…" : "同步我的皮肤库"}
            title="手动同步会下载云端个人皮肤、上传本机皮肤，并同步已记录的删除。"
          >
            {sync.busy ? "正在同步…" : "同步皮肤库"}
          </button>
          {localSkins && (
            <ActionButton
              action={() => setPublishOpen(true)}
              className="primary"
              label="发布皮肤"
              ariaLabel="发布我的皮肤"
            />
          )}
        </div>
      </div>
      <CommunityGalleryFeedback
        error={sync.error ? candidateSkinMessage(sync.error) : undefined}
        signInRequired={communityNeedsSignIn(sync.error)}
        onLogin={onLogin}
        notice={
          sync.report && (
            <CommunityActionNotice>
              {sync.report.stopped ? "同步已暂停" : "同步已完成"}：下载{" "}
              {sync.report.downloaded.length} 款，上传 {sync.report.uploaded.length} 款，本机删除{" "}
              {sync.report.deleted_local.length} 款，云端删除 {sync.report.deleted_cloud.length}{" "}
              款，跳过 {sync.report.skipped.length} 款。
            </CommunityActionNotice>
          )
        }
      />
      <CommunitySkinCategoryFilter
        ariaLabel="候选窗口皮肤分类"
        value={categoryFilter.category}
        onChange={(next) => void changeCategory(next)}
      />
      <CommunityGalleryFeedback
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
        notice={actionNotice && <CommunityActionNotice>{actionNotice}</CommunityActionNotice>}
        empty={
          !listBusy && skins.length === 0 ? (
            <CommunityNotice>
              {mineOnly ? "你的皮肤库里还没有候选窗口皮肤。" : "暂时没有匹配的候选窗口皮肤。"}
            </CommunityNotice>
          ) : undefined
        }
      />
      <CommunityGalleryGrid>
        {skins.map((skin) => (
          <CommunityCandidateSkinCard
            key={skin.id}
            skin={skin}
            load={loadPreview}
            open={() => open(skin)}
          />
        ))}
      </CommunityGalleryGrid>
      <CommunityGalleryLoadMore
        hasMore={hasMore}
        busy={listBusy}
        loadingText="正在读取候选窗口皮肤…"
        onLoadMore={() => void requestList(activeSearch, true)}
      />
      {publishOpen && localSkins && (
        <CandidateSkinPublishDialog
          client={client}
          localSkins={localSkins}
          openSkinDirectory={openSkinDirectory}
          readImage={readSkinImage}
          onClose={() => setPublishOpen(false)}
          onPublished={publishDone}
          onLogin={communityPublishLoginAction(() => setPublishOpen(false), onLogin)}
        />
      )}
    </CommunityPageShell>
  );
}
