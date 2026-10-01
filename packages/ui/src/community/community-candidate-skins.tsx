import { useEffect, useMemo, useState } from "react";
import { errorCode } from "../core/error-code";
import type { SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { CandidateSkinPublishDialog } from "./candidate-skin-publish-dialog";
import { candidateSkinMessage, communityNeedsSignIn } from "./community-helpers";
import { useCommunityGallery, type CommunityGalleryClient } from "./community-gallery";
import { CommunityErrorAlert } from "./community-error-alert";
import { CommunityDetailStatus } from "./community-detail-status";
import * as style from "./community-style";
import { CommunitySearchForm } from "./community-search-form";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunitySkinModerationSection } from "./community-skin-moderation-section";
import {
  CommunityRemovedBadge,
  CommunityReportSection,
  type CommunityModeration,
  type CommunityReportReason,
} from "./community-report";
import { CommunitySkinCardMetrics } from "./community-skin-card-metrics";
import { CommunityInstallButton } from "./community-install-button";
import { CommunityReplaceConfirmation } from "./community-replace-confirmation";
import { CommunityBackButton } from "./community-gallery-controls";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import {
  CommunitySkinCategoryFilter,
  CommunitySkinCategorySelect,
  communitySkinCategories,
  communitySkinCategoryLabel,
  communitySkinCategoryLabels,
  useCommunitySkinCategoryFilter,
  type CommunitySkinCategory,
} from "./community-skin-category";

/** The server's license columns; each is `""` when the manifest leaves it out. */
export type CommunityCandidateSkinLicense = {
  code: string;
  assets: string;
  source: string;
};

/** Who can see a package: everyone in the gallery, or only its owner, whose library sync keeps private packages in. */
export type CandidateSkinVisibility = "public" | "private";

/** 候选窗皮肤的发布分类，与社区键盘皮肤共用；只是发布元数据，不写进 skin.toml。 */
export type CandidateSkinCategory = CommunitySkinCategory;
export const candidateSkinCategories = communitySkinCategories;
export const candidateSkinCategoryLabels = communitySkinCategoryLabels;

export type CommunityCandidateSkin = {
  id: string;
  /** The manifest id, which is also the folder the package installs into. */
  package_id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  license: CommunityCandidateSkinLicense;
  size: number;
  file_count: number;
  downloads: number;
  rating_count: number;
  rating_average: number;
  owned: boolean;
  my_rating: number;
  created_at: string;
  visibility: CandidateSkinVisibility;
  updated_at: string;
  /** Sent only on the user's own packages; `removed` shows 已下架. */
  moderation?: CommunityModeration | null;
  /** 发布分类；早于分类功能的服务端不返回。 */
  category?: CandidateSkinCategory;
};

export type CommunityCandidateSkinPage = {
  skins: CommunityCandidateSkin[];
  has_more: boolean;
};

/** What the host's packer found in a local package before anything is uploaded; the license comes from skin.toml, so absent keys are `null`. */
export type CandidateSkinPackPreview = {
  suggestedName: string;
  license: {
    code: string | null;
    assets: string | null;
    source: string | null;
  };
  fileCount: number;
  size: number;
};

/** A package one sync run left as it was, with a `candidate_skin_*` rule code or an `account_*` request code. */
export type CandidateSkinSyncSkip = { package_id: string; code: string };

/** What one sync of the local skin directory with the user's library did, each list by package id. */
export type CandidateSkinSyncReport = {
  uploaded: string[];
  downloaded: string[];
  deleted_local: string[];
  deleted_cloud: string[];
  skipped: CandidateSkinSyncSkip[];
  /** `account_rate_limited` or `candidate_skin_library_limit` when uploads stopped for the rest of the run. */
  stopped: string | null;
};

/** Desktop community commands for candidate-window skin packages; the host packs, downloads and installs, so the webview never handles paths or package bytes. */
export interface CandidateSkinCommunityClient {
  /** `category` 为 `null` 时列出全部分类。 */
  list(
    offset: number,
    search: string,
    mine: boolean,
    category: CandidateSkinCategory | null,
  ): Promise<CommunityCandidateSkinPage>;
  detail(id: string): Promise<CommunityCandidateSkin>;
  preview(id: string): Promise<{ dataUrl: string }>;
  /** Downloads and installs into the external skin directory, answering with the rescanned catalog. */
  install(id: string, replace: boolean): Promise<SkinCatalog>;
  /** Checks the package against the rules for `visibility`: only a public package needs an asset license. */
  packPreview(
    skinId: string,
    visibility: CandidateSkinVisibility,
  ): Promise<CandidateSkinPackPreview>;
  /** Saves `bytes`, a PNG or JPEG preview the page drew, into the installed package `skinId`, which has none, and answers with the rescanned catalog. */
  addPreview(skinId: string, bytes: number[]): Promise<SkinCatalog>;
  /** Writes `assets`, the asset license the user chose, into the manifest of the installed package `skinId`, which has none, and answers with the rescanned catalog. */
  addLicense(skinId: string, assets: string): Promise<SkinCatalog>;
  /** A package sync already keeps in the library is updated in place, so publishing never leaves a second copy. */
  publish(
    skinId: string,
    id: string,
    name: string,
    description: string,
    visibility: CandidateSkinVisibility,
    category: CandidateSkinCategory,
  ): Promise<CommunityCandidateSkin>;
  setVisibility(id: string, visibility: CandidateSkinVisibility): Promise<CommunityCandidateSkin>;
  /** 修改自己作品的发布分类。 */
  setCategory(id: string, category: CandidateSkinCategory): Promise<CommunityCandidateSkin>;
  /** Brings the local skin directory and the signed-in user's library in step; `community_unauthorized` when signed out. */
  sync(): Promise<CandidateSkinSyncReport>;
  rate(id: string, stars: number): Promise<{ stars: number }>;
  unpublish(id: string): Promise<{ deleted: boolean }>;
  /** Reports another user's package to the moderators. */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}

type PreviewLoader = (id: string) => Promise<string>;

function licenseLine(license: CommunityCandidateSkinLicense): string {
  return [
    license.assets.trim() ? `素材授权 ${license.assets.trim()}` : "",
    license.code.trim() ? `代码授权 ${license.code.trim()}` : "",
    license.source.trim() ? `来源 ${license.source.trim()}` : "",
  ]
    .filter(Boolean)
    .join(" / ");
}

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
  useEffect(() => {
    let active = true;
    setUrl("");
    setFailed(false);
    void load(id)
      .then((value) => {
        if (active) setUrl(value);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [id, load]);
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
    <button
      type="button"
      className={style.card}
      aria-label={`查看候选窗皮肤 ${skin.name}`}
      onClick={open}
    >
      <CandidateSkinPreviewImage
        id={skin.id}
        name={skin.name}
        load={load}
        className={style.cardStage}
      />
      <strong className={style.cardTitle}>{skin.name}</strong>
      <span className={style.cardAuthor}>
        {communitySkinCategoryLabel(skin.category) &&
          `${communitySkinCategoryLabel(skin.category)} · `}
        {skin.owned ? "我的作品" : skin.author}
        {skin.visibility === "private" && " · 私有"}
        {skin.owned && skin.moderation === "removed" && " · 已下架"}
      </span>
      <CommunitySkinCardMetrics
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
      />
      {skin.license.assets.trim() && (
        <span className={style.cardAuthor}>素材授权 {skin.license.assets.trim()}</span>
      )}
    </button>
  );
}

/** The community gallery of candidate-window skin packages: browse, install into the external skin directory, rate, publish and take down. */
export function CommunityCandidateSkinsPage({
  client,
  localSkins,
  openSkinDirectory,
  readSkinImage,
  onOpenSkinPage,
  onInstalled,
  onLogin,
}: {
  client: CandidateSkinCommunityClient;
  /** The installed packages: the publish choices, and how an install learns it would replace one. */
  localSkins?: () => Promise<SkinCatalog>;
  openSkinDirectory?: () => Promise<void>;
  readSkinImage?: SkinImageReader;
  /** Opens 主题, where an installed package is enabled; the community page sits outside the settings form and never writes preferences itself. */
  onOpenSkinPage?: () => void;
  /** Called once a package lands in the external skin directory, so a listing of that directory can scan again. */
  onInstalled?: () => void;
  onLogin?: () => void;
}) {
  const categoryFilter = useCommunitySkinCategoryFilter();
  const categoryRequest = categoryFilter.request;
  const galleryClient = useMemo<CommunityGalleryClient<CommunityCandidateSkin>>(
    () => ({
      list: async (offset, search, mine) => {
        const page = await client.list(offset, search, mine ?? false, categoryRequest.current);
        return { items: page.skins, has_more: page.has_more };
      },
      detail: client.detail,
      rate: async (id, stars) => {
        await client.rate(id, stars);
      },
      unpublish: async (id) => {
        await client.unpublish(id);
      },
      ...(client.report && {
        report: (id: string, reason: CommunityReportReason, detail: string) =>
          client.report!(id, reason, detail),
      }),
    }),
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
    setMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail: closeGalleryDetail,
    rateSelected,
    unpublishSelected,
    reportSelected,
    runAction,
  } = gallery;
  const [search, setSearch] = useState("");
  const [installed, setInstalled] = useState(false);
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);
  const changeCategory = (next: CandidateSkinCategory | null) =>
    categoryFilter.change(next, () => requestList(activeSearch, false));

  // One read per package per client: a card and its detail view share the image rather than fetching it twice. The cache belongs to the client, so a replaced client never answers with the previous one's image.
  const loadPreview = useMemo<PreviewLoader>(() => {
    const cache = new Map<string, Promise<string>>();
    return (id) => {
      let pending = cache.get(id);
      if (!pending) {
        pending = client.preview(id).then((value) => value.dataUrl);
        pending.catch(() => cache.delete(id));
        cache.set(id, pending);
      }
      return pending;
    };
  }, [client]);

  const closeDetail = () => {
    if (actionBusy) return;
    closeGalleryDetail();
    setInstalled(false);
    setConfirmReplace(false);
  };

  const install = async (replace: boolean) => {
    if (!selected) return;
    const target = selected;
    setInstalled(false);
    await runAction(
      async (currentClient) => {
        // Asking before the download spares a ~3 MB transfer the user may be about to refuse. The scan is only a courtesy: when it fails the install's own `candidate_skin_exists` still stops an overwrite.
        if (!replace && localSkins) {
          const catalog = await localSkins().catch(() => null);
          if (!gallery.isCurrent(currentClient)) return;
          if (catalog?.packages.some((item) => item.id === target.package_id)) {
            setConfirmReplace(true);
            return;
          }
        }
        await client.install(target.id, replace);
        if (!gallery.isCurrent(currentClient)) return;
        setConfirmReplace(false);
        setInstalled(true);
        onInstalled?.();
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
    if (!selected) return;
    const target = selected;
    await runAction(
      async (currentClient) => {
        const updated = await client.setVisibility(target.id, visibility);
        if (!gallery.isCurrent(currentClient)) return;
        gallery.setSelected(updated);
        gallery.setItems((current) =>
          current.map((item) => (item.id === updated.id ? updated : item)),
        );
        setActionNotice(
          visibility === "public"
            ? "已公开，其他用户现在可以下载这款皮肤。"
            : "已设为私有，只有你能看到这款皮肤。",
        );
      },
      { clearNotice: true },
    );
  };

  const changeOwnCategory = async (next: CandidateSkinCategory) => {
    if (!selected) return;
    const target = selected;
    await runAction(
      async (currentClient) => {
        const updated = await client.setCategory(target.id, next);
        if (!gallery.isCurrent(currentClient)) return;
        gallery.setSelected(updated);
        gallery.setItems((current) =>
          current.map((item) => (item.id === updated.id ? updated : item)),
        );
        setActionNotice(`已改为「${candidateSkinCategoryLabels[next]}」分类。`);
      },
      { clearNotice: true },
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

  const errorAlert = error && (
    <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
  );

  if (selected) {
    const license = licenseLine(selected.license);
    const selectedCategory = communitySkinCategoryLabel(selected.category);
    return (
      <div className={style.page}>
        <CommunityBackButton disabled={actionBusy} onClick={closeDetail} />
        {errorAlert}
        <section className={`section ${style.detail}`}>
          <CandidateSkinPreviewImage
            id={selected.id}
            name={selected.name}
            load={loadPreview}
            className={style.detailStage}
          />
          <div className={style.detailTitle}>
            <div className={style.headingBody}>
              <h2 className={style.headingTitle}>{selected.name}</h2>
              <p className={style.headingNote}>
                {[selectedCategory, selected.author, selected.version && `v${selected.version}`]
                  .filter(Boolean)
                  .join(" · ")}
              </p>
            </div>
            {selected.owned && (
              <span className={style.detailBadge}>
                {selected.visibility === "private" ? "私有" : "我的作品"}
              </span>
            )}
            <CommunityRemovedBadge owned={selected.owned} moderation={selected.moderation} />
          </div>
          {selected.description && <p className={style.description}>{selected.description}</p>}
          {license && <p className={style.metrics}>{license}</p>}
          <CommunityDetailStatus
            downloads={selected.downloads}
            ratingCount={selected.rating_count}
            ratingAverage={selected.rating_average}
            myRating={selected.my_rating}
            detailBusy={detailBusy}
            actionNotice={actionNotice}
            loadingText="正在读取皮肤详情…"
          />
          {installed ? (
            <>
              <p role="status" className={style.actionNotice}>
                已安装到外部皮肤。
              </p>
              {onOpenSkinPage && (
                <button
                  type="button"
                  className={`primary ${style.action}`}
                  onClick={onOpenSkinPage}
                >
                  去启用
                </button>
              )}
            </>
          ) : (
            <CommunityInstallButton
              actionBusy={actionBusy}
              detailBusy={detailBusy}
              confirmReplace={confirmReplace}
              onInstall={() => void install(false)}
            />
          )}
          {confirmReplace && (
            <CommunityReplaceConfirmation
              ariaLabel="确认替换皮肤"
              message={<>已存在同名皮肤“{selected.package_id}”，安装会整体替换它。</>}
              actionBusy={actionBusy}
              onConfirm={() => void install(true)}
              onCancel={() => setConfirmReplace(false)}
            />
          )}
          {selected.owned && (
            <button
              type="button"
              className={`secondary ${style.action}`}
              disabled={actionBusy || detailBusy}
              onClick={() =>
                void changeVisibility(selected.visibility === "private" ? "public" : "private")
              }
            >
              {selected.visibility === "private" ? "公开" : "设为私有"}
            </button>
          )}
          {selected.owned && (
            <CommunitySkinCategorySelect
              ariaLabel="修改分类"
              value={selected.category ?? "other"}
              disabled={actionBusy || detailBusy}
              onChange={(next) => void changeOwnCategory(next)}
            />
          )}
          <CommunitySkinModerationSection
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
            confirmationActionsClassName={style.confirmationActions}
          />
          {!selected.owned && client.report && (
            <CommunityReportSection actionBusy={actionBusy} onReport={reportSelected} />
          )}
        </section>
      </div>
    );
  }

  return (
    <div className={style.page}>
      <CommunitySearchForm
        label="搜索候选窗皮肤"
        value={search}
        onChange={setSearch}
        onSubmit={() => void requestList(search, false)}
      />
      <div className={style.heading}>
        <div className={style.headingBody}>
          <h2 className={style.headingTitle}>候选窗皮肤</h2>
          <p className={style.headingNote}>为输入候选窗换一身新装，下载后在「主题」中启用</p>
        </div>
        <div className={style.headingActions}>
          <CommunityScopeButtons
            ariaLabel="候选窗皮肤范围"
            mineOnly={mineOnly}
            allLabel="全部"
            mineLabel="我的作品"
            onMineOnlyChange={(nextMineOnly) => {
              setMineOnly(nextMineOnly);
              void requestList(activeSearch, false, nextMineOnly);
            }}
          />
          {localSkins && (
            <button type="button" className="primary" onClick={() => setPublishOpen(true)}>
              发布我的皮肤
            </button>
          )}
        </div>
      </div>
      <CommunitySkinCategoryFilter
        ariaLabel="候选窗皮肤分类"
        value={categoryFilter.category}
        onChange={(next) => void changeCategory(next)}
      />
      {errorAlert}
      {actionNotice && (
        <p role="status" className={style.actionNotice}>
          {actionNotice}
        </p>
      )}
      {!listBusy && skins.length === 0 && (
        <p className={style.notice}>
          {mineOnly ? "你的皮肤库里还没有候选窗皮肤。" : "暂时没有匹配的候选窗皮肤。"}
        </p>
      )}
      <div className={style.grid}>
        {skins.map((skin) => (
          <CommunityCandidateSkinCard
            key={skin.id}
            skin={skin}
            load={loadPreview}
            open={() => open(skin)}
          />
        ))}
      </div>
      <CommunityGalleryLoadMore
        hasMore={hasMore}
        busy={listBusy}
        loadingText="正在读取候选窗皮肤…"
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
          onLogin={
            onLogin &&
            (() => {
              setPublishOpen(false);
              onLogin();
            })
          }
        />
      )}
    </div>
  );
}
