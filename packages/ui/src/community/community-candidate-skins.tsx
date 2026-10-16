import { useEffect, useMemo, useState } from "react";
import { errorCode } from "../core/error-code";
import type { SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { notifySkinCatalogChanged } from "../skin/skin-catalog-changes";
import { CandidateSkinPublishDialog } from "./candidate-skin-publish-dialog";
import { useCandidateSkinSync } from "./candidate-skin-sync";
import {
  candidateSkinMessage,
  communityLicenseLine,
  communityNeedsSignIn,
  communityPublishLoginAction,
} from "./community-helpers";
import {
  createCommunityGalleryClient,
  useCommunityGallery,
  type CommunityGalleryClient,
} from "./community-gallery";
import { CommunityDetailSummary } from "./community-detail-summary";
import { CommunityDetailFrame } from "./community-detail-frame";
import * as style from "./community-style";
import { CommunityGalleryHeader } from "./community-gallery-header";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityReportSection } from "./community-report";
import { CommunityCardMetrics } from "./community-card-metrics";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCard } from "./community-card";
import { CommunityMetrics } from "./community-metrics";
import { CommunityInstallActions } from "./community-install-actions";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { CommunityNotice } from "./community-notice";
import { useCommunityInstallState } from "./community-install-state";
import { communityInstallPreflight } from "./community-install-preflight";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { ActionButton } from "../core/action-button";
import {
  CommunitySkinCategoryFilter,
  CommunitySkinCategorySelect,
} from "./community-skin-category";
import { communitySkinCategoryLabel } from "./community-skin-category-model";
import { useCommunitySkinCategoryFilter } from "./use-community-skin-category-filter";

import {
  type CandidateSkinVisibility,
  type CandidateSkinCategory,
  candidateSkinCategoryLabels,
  type CommunityCandidateSkin,
  type CandidateSkinCommunityClient,
} from "./community-candidate-skin-model";
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

type PreviewLoader = (id: string) => Promise<string>;
const CANDIDATE_SKIN_PREVIEW_CACHE_CAPACITY = 16;

/** The package's preview image, read only once its card or detail view is on the page. */
function CandidateSkinPreviewImage({
  id,
  name,
  load,
  className,
}: {
  id: string;
  name: string;
  load: PreviewLoader;
  className: string;
}) {
  const [url, setUrl] = useState("");
  const [failed, setFailed] = useState(false);
  const generation = useAsyncGeneration(id, load);
  useEffect(() => {
    const requestGeneration = generation.current;
    setUrl("");
    setFailed(false);
    void load(id)
      .then((value) => {
        if (generation.current === requestGeneration) setUrl(value);
      })
      .catch(() => {
        if (generation.current === requestGeneration) setFailed(true);
      });
  }, [id, load, generation]);
  return (
    <span className={`${className} grid min-h-[96px] place-items-center`}>
      {url ? (
        <img className="block max-h-[220px] w-full object-contain" src={url} alt={`${name} 预览`} />
      ) : (
        <span className="text-xs [color:var(--p-sub)]">
          {failed ? "预览图加载失败" : "正在读取预览…"}
        </span>
      )}
    </span>
  );
}

function CommunityCandidateSkinCard({
  skin,
  load,
  open,
}: {
  skin: CommunityCandidateSkin;
  load: PreviewLoader;
  open: () => void;
}) {
  return (
    <CommunityCard aria-label={`查看候选窗口皮肤 ${skin.name}`} onClick={open}>
      <CandidateSkinPreviewImage
        id={skin.id}
        name={skin.name}
        load={load}
        className={style.cardStage}
      />
      <strong className={style.cardTitle}>{skin.name}</strong>
      <CommunityCardAuthor
        prefix={communitySkinCategoryLabel(skin.category) ?? undefined}
        author={skin.author}
        owned={skin.owned}
        private={skin.visibility === "private"}
        removed={skin.moderation === "removed"}
      />
      <CommunityCardMetrics
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
      />
      {skin.license.assets.trim() && (
        <span className={style.cardAuthor}>素材授权 {skin.license.assets.trim()}</span>
      )}
    </CommunityCard>
  );
}

/** The community gallery of candidate-window skin packages: browse, install into the external skin directory, rate, publish and take down. */
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
  /** Returns to 我的皮肤 on 主题, where an installed package is enabled; the community page sits outside the settings form and never writes preferences itself. */
  onOpenSkinPage?: () => void;
  onLogin?: () => void;
}) {
  const sync = useCandidateSkinSync(client, notifySkinCatalogChanged);
  const categoryFilter = useCommunitySkinCategoryFilter();
  const categoryRequest = categoryFilter.request;
  const galleryClient = useMemo<CommunityGalleryClient<CommunityCandidateSkin>>(
    () =>
      createCommunityGalleryClient(
        client,
        async (offset, search, mine) => {
          const page = await client.list(offset, search, mine ?? false, categoryRequest.current);
          return { items: page.skins, has_more: page.has_more };
        },
        (id) => client.unpublish(id),
      ),
    [client, categoryRequest],
  );
  const gallery = useCommunityGallery({
    client: galleryClient,
    errorMessage: candidateSkinMessage,
    needsSignIn: communityNeedsSignIn,
  });
  const {
    items: skins,
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
    setActionNotice,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail: closeGalleryDetail,
    rateSelected,
    updateSelected,
    unpublishSelected,
    reportSelected,
    runAction,
  } = gallery;
  const [search, setSearch] = useState("");
  const [publishOpen, setPublishOpen] = useState(false);
  const { installed, setInstalled, confirmReplace, setConfirmReplace, closeDetail } =
    useCommunityInstallState(actionBusy, closeGalleryDetail);
  const changeCategory = (next: CandidateSkinCategory | null) =>
    categoryFilter.change(next, () => requestList(activeSearch, false));

  // One read per package per client: a card and its detail view share the image rather than fetching it twice. The cache belongs to the client, so a replaced client never answers with the previous one's image.
  const loadPreview = useMemo<PreviewLoader>(() => {
    const cache = new Map<string, Promise<string>>();
    return (id) => {
      const cached = cache.get(id);
      if (cached) {
        cache.delete(id);
        cache.set(id, cached);
        return cached;
      }
      const pending = client.preview(id).then((value) => value.dataUrl);
      pending.catch(() => {
        if (cache.get(id) === pending) cache.delete(id);
      });
      cache.set(id, pending);
      while (cache.size > CANDIDATE_SKIN_PREVIEW_CACHE_CAPACITY) {
        const oldest = cache.keys().next().value;
        if (oldest === undefined) break;
        cache.delete(oldest);
      }
      return pending;
    };
  }, [client]);

  const install = async (replace: boolean) => {
    if (!selected) return;
    const target = selected;
    setInstalled(false);
    await runAction(
      async (currentClient) => {
        // Asking before the download spares a ~3 MB transfer the user may be about to refuse. The scan is only a courtesy: when it fails the install's own `candidate_skin_exists` still stops an overwrite.
        if (!replace && localSkins) {
          const preflight = await communityInstallPreflight(
            localSkins,
            () => gallery.isCurrent(currentClient),
            (catalog) => catalog.packages.some((item) => item.id === target.package_id),
          );
          if (preflight === "stale") return;
          if (preflight === "conflict") {
            setConfirmReplace(true);
            return;
          }
        }
        await client.install(target.id, replace);
        // 皮肤已经写进目录，即使图库随后换了客户端，主题页也该重扫。
        notifySkinCatalogChanged();
        if (!gallery.isCurrent(currentClient)) return;
        setConfirmReplace(false);
        setInstalled(true);
      },
      {
        clearNotice: true,
        ignoreError: (actionError) => errorCode(actionError) === "candidate_skin_exists",
        onError: (actionError) => {
          if (errorCode(actionError) === "candidate_skin_exists") setConfirmReplace(true);
        },
      },
    );
  };

  const changeVisibility = async (visibility: CandidateSkinVisibility) => {
    await updateSelected(
      (id) => client.setVisibility(id, visibility),
      visibility === "public"
        ? "已公开，其他用户现在可以下载这款皮肤。"
        : "已设为私有，只有你能看到这款皮肤。",
    );
  };

  const changeOwnCategory = async (next: CandidateSkinCategory) => {
    await updateSelected(
      (id) => client.setCategory(id, next),
      `已改为「${candidateSkinCategoryLabels[next]}」分类。`,
    );
  };

  const unpublish = () => {
    void unpublishSelected(
      "已下架这款皮肤；其他用户将无法再下载。本地皮肤会保留，并在下次同步时作为私有皮肤存回你的皮肤库。",
    );
  };

  const publishDone = async (published: CommunityCandidateSkin) => {
    setPublishOpen(false);
    setActionNotice(
      published.visibility === "private" ? "已保存到你的皮肤库，仅自己可见。" : "已发布到社区。",
    );
    await requestList(activeSearch, false);
  };

  if (selected) {
    const license = communityLicenseLine(selected.license);
    const selectedCategory = communitySkinCategoryLabel(selected.category);
    return (
      <CommunityDetailFrame
        backDisabled={actionBusy}
        onBack={closeDetail}
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
      >
        <CandidateSkinPreviewImage
          id={selected.id}
          name={selected.name}
          load={loadPreview}
          className={style.detailStage}
        />
        <CommunityDetailSummary
          title={selected.name}
          noteParts={[
            selectedCategory,
            selected.author,
            selected.version && `v${selected.version}`,
          ]}
          owned={selected.owned}
          moderation={selected.moderation}
          ownedLabel={selected.visibility === "private" ? "私有" : "我的作品"}
          description={selected.description}
          metrics={license && <CommunityMetrics>{license}</CommunityMetrics>}
          downloads={selected.downloads}
          ratingCount={selected.rating_count}
          ratingAverage={selected.rating_average}
          myRating={selected.my_rating}
          detailBusy={detailBusy}
          actionNotice={actionNotice}
          loadingText="正在读取皮肤详情…"
        />
        <CommunityInstallActions
          installed={installed}
          installedContent={
            <>
              <CommunityActionNotice>已安装到外部皮肤。</CommunityActionNotice>
              {onOpenSkinPage && (
                <ActionButton
                  action={onOpenSkinPage}
                  className={`primary ${style.action}`}
                  label="去启用"
                />
              )}
            </>
          }
          actionBusy={actionBusy}
          detailBusy={detailBusy}
          confirmReplace={confirmReplace}
          replaceAriaLabel="确认替换皮肤"
          replaceMessage={<>已存在同名皮肤“{selected.package_id}”，安装会整体替换它。</>}
          onInstall={(replace) => void install(replace)}
          onCancelReplace={() => setConfirmReplace(false)}
        />
        {selected.owned && (
          <ActionButton
            action={() =>
              void changeVisibility(selected.visibility === "private" ? "public" : "private")
            }
            className={`secondary ${style.action}`}
            disabled={actionBusy || detailBusy}
            label={selected.visibility === "private" ? "公开" : "设为私有"}
          />
        )}
        {selected.owned && (
          <CommunitySkinCategorySelect
            ariaLabel="修改分类"
            value={selected.category ?? "other"}
            disabled={actionBusy || detailBusy}
            onChange={(next) => void changeOwnCategory(next)}
          />
        )}
        <CommunityModerationSection
          owned={selected.owned}
          unpublishable={selected.visibility === "public"}
          actionBusy={actionBusy}
          ratingDescription="我的评分（安装后可评，可重新选择）"
          unpublishMessage={`下架后其他用户无法再下载，下载数和评分会清空；本地皮肤会保留并以私有方式同步。只想不让别人看到，可以改用“设为私有”。确定下架“${selected.name}”吗？`}
          confirmUnpublish={confirmUnpublish}
          onRate={(stars) => void rateSelected(stars)}
          onRequestUnpublish={() => setConfirmUnpublish(true)}
          onUnpublish={() => void unpublish()}
          onCancelUnpublish={() => setConfirmUnpublish(false)}
        />
        {!selected.owned && client.report && (
          <CommunityReportSection actionBusy={actionBusy} onReport={reportSelected} />
        )}
      </CommunityDetailFrame>
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
        note="为输入候选窗口换一身新装，安装后在「我的皮肤」中启用"
      >
        <CommunityScopeButtons
          ariaLabel="候选窗口皮肤范围"
          mineOnly={mineOnly}
          allLabel="全部"
          mineLabel="我的作品"
          onMineOnlyChange={changeMineOnly}
        />
        {localSkins && (
          <ActionButton
            action={() => setPublishOpen(true)}
            className="primary"
            label="发布我的皮肤"
          />
        )}
        <ActionButton
          action={sync.run}
          label={sync.busy ? "正在同步皮肤库…" : "同步我的皮肤库"}
          disabled={sync.busy}
          ariaBusy={sync.busy}
        />
      </CommunityGalleryHeader>
      <p className="text-xs [color:var(--p-sub)]">
        手动同步会下载云端个人皮肤、上传本机皮肤，并同步已记录的删除。
      </p>
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
