import { useEffect, useMemo, useState } from "react";
import { errorCode } from "../core/error-code";
import type { SkinCatalog } from "../skin/external-skins";
import { CandidateSkinPublishDialog } from "./candidate-skin-publish-dialog";
import { candidateSkinMessage, communityNeedsSignIn, communityRating } from "./community-helpers";
import { useCommunityGallery, type CommunityGalleryClient } from "./community-gallery";
import * as style from "./community-style";
import { CommunitySearchForm } from "./community-search-form";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunitySkinModerationSection } from "./community-skin-moderation-section";

/** The server's license columns; each is `""` when the manifest leaves it out. */
export type CommunityCandidateSkinLicense = { code: string; assets: string; source: string };

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
};

export type CommunityCandidateSkinPage = {
  skins: CommunityCandidateSkin[];
  has_more: boolean;
};

/** What the host's packer found in a local package before anything is uploaded; the license comes from skin.toml, so absent keys are `null`. */
export type CandidateSkinPackPreview = {
  suggestedName: string;
  license: { code: string | null; assets: string | null; source: string | null };
  fileCount: number;
  size: number;
};

/** Desktop community commands for candidate-window skin packages; the host packs, downloads and installs, so the webview never handles paths or package bytes. */
export interface CandidateSkinCommunityClient {
  list(offset: number, search: string, mine: boolean): Promise<CommunityCandidateSkinPage>;
  detail(id: string): Promise<CommunityCandidateSkin>;
  preview(id: string): Promise<{ dataUrl: string }>;
  /** Downloads and installs into the external skin directory, answering with the rescanned catalog. */
  install(id: string, replace: boolean): Promise<SkinCatalog>;
  packPreview(skinId: string): Promise<CandidateSkinPackPreview>;
  publish(
    skinId: string,
    id: string,
    name: string,
    description: string,
  ): Promise<CommunityCandidateSkin>;
  rate(id: string, stars: number): Promise<{ stars: number }>;
  unpublish(id: string): Promise<{ deleted: boolean }>;
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
      <span className={style.cardAuthor}>{skin.owned ? "我的作品" : skin.author}</span>
      <span className={style.cardMetrics}>
        <span>↓ {skin.downloads.toLocaleString("zh-CN")}</span>
        <span>☆ {communityRating(skin.rating_count, skin.rating_average)}</span>
      </span>
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
  onOpenSkinPage,
  onInstalled,
  onLogin,
}: {
  client: CandidateSkinCommunityClient;
  /** The installed packages: the publish choices, and how an install learns it would replace one. */
  localSkins?: () => Promise<SkinCatalog>;
  openSkinDirectory?: () => Promise<void>;
  /** Opens 主题, where an installed package is enabled; the community page sits outside the settings form and never writes preferences itself. */
  onOpenSkinPage?: () => void;
  /** Called once a package lands in the external skin directory, so a listing of that directory can scan again. */
  onInstalled?: () => void;
  onLogin?: () => void;
}) {
  const galleryClient = useMemo<CommunityGalleryClient<CommunityCandidateSkin>>(
    () => ({
      list: async (offset, search, mine) => {
        const page = await client.list(offset, search, mine ?? false);
        return { items: page.skins, has_more: page.has_more };
      },
      detail: client.detail,
      rate: async (id, stars) => {
        await client.rate(id, stars);
      },
      unpublish: async (id) => {
        await client.unpublish(id);
      },
    }),
    [client],
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
  } = gallery;
  const [search, setSearch] = useState("");
  const [installed, setInstalled] = useState(false);
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);

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
    const currentClient = gallery.beginAction();
    if (currentClient === null) return;
    const target = selected;
    gallery.setError("");
    setActionNotice("");
    setInstalled(false);
    try {
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
    } catch (actionError) {
      if (!gallery.isCurrent(currentClient)) return;
      if (errorCode(actionError) === "candidate_skin_exists") {
        setConfirmReplace(true);
        return;
      }
      gallery.fail(actionError);
    } finally {
      gallery.endAction(currentClient);
    }
  };

  const unpublish = () => {
    void unpublishSelected("已下架这款皮肤；其他用户将无法再下载，已安装的本地副本不会受影响。");
  };

  const publishDone = async () => {
    setPublishOpen(false);
    setActionNotice("已发布到社区。");
    await requestList(activeSearch, false);
  };

  const errorAlert = error && (
    <p role="alert" className="error">
      {error}
      {signInRequired && onLogin && (
        <>
          {" "}
          <button type="button" className="secondary" onClick={onLogin}>
            去登录
          </button>
        </>
      )}
    </p>
  );

  if (selected) {
    const license = licenseLine(selected.license);
    return (
      <div className={style.page}>
        <button
          type="button"
          className={style.back}
          disabled={actionBusy}
          onClick={closeDetail}
          aria-label="返回社区"
        >
          ← 社区
        </button>
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
                {[selected.author, selected.version && `v${selected.version}`]
                  .filter(Boolean)
                  .join(" · ")}
              </p>
            </div>
            {selected.owned && <span className={style.detailBadge}>我的作品</span>}
          </div>
          {selected.description && <p className={style.description}>{selected.description}</p>}
          {license && <p className={style.metrics}>{license}</p>}
          <p className={style.metrics}>
            {selected.downloads.toLocaleString("zh-CN")} 人下载 ·{" "}
            {communityRating(selected.rating_count, selected.rating_average)} ·{" "}
            {selected.rating_count.toLocaleString("zh-CN")} 人评分
          </p>
          {selected.my_rating > 0 && (
            <p className={style.metrics}>我的评分：{selected.my_rating} 星</p>
          )}
          {detailBusy && <p role="status">正在读取皮肤详情…</p>}
          {actionNotice && (
            <p role="status" className={style.actionNotice}>
              {actionNotice}
            </p>
          )}
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
            <button
              type="button"
              className={`primary ${style.action}`}
              disabled={actionBusy || detailBusy || confirmReplace}
              onClick={() => void install(false)}
            >
              {actionBusy ? "正在安装…" : "一键安装"}
            </button>
          )}
          {confirmReplace && (
            <div className={style.confirmation} role="alertdialog" aria-label="确认替换皮肤">
              <p>已存在同名皮肤“{selected.package_id}”，安装会整体替换它。</p>
              <div className={style.confirmationActions}>
                <button
                  type="button"
                  className="danger"
                  disabled={actionBusy}
                  onClick={() => void install(true)}
                >
                  替换安装
                </button>
                <button
                  type="button"
                  className="secondary"
                  disabled={actionBusy}
                  onClick={() => setConfirmReplace(false)}
                >
                  取消
                </button>
              </div>
            </div>
          )}
          <CommunitySkinModerationSection
            owned={selected.owned}
            actionBusy={actionBusy}
            ratingDescription="我的评分（安装后可评，可重新选择）"
            unpublishMessage={`下架后其他用户无法再下载，已安装的本地皮肤会保留。确定下架“${selected.name}”吗？`}
            confirmUnpublish={confirmUnpublish}
            onRate={(stars) => void rateSelected(stars)}
            onRequestUnpublish={() => setConfirmUnpublish(true)}
            onUnpublish={() => void unpublish()}
            onCancelUnpublish={() => setConfirmUnpublish(false)}
            confirmationActionsClassName={style.confirmationActions}
          />
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
      {errorAlert}
      {actionNotice && (
        <p role="status" className={style.actionNotice}>
          {actionNotice}
        </p>
      )}
      {!listBusy && skins.length === 0 && (
        <p className={style.notice}>
          {mineOnly ? "还没有已发布的候选窗皮肤。" : "暂时没有匹配的候选窗皮肤。"}
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
      {hasMore && (
        <button
          type="button"
          className={`secondary ${style.more}`}
          disabled={listBusy}
          onClick={() => void requestList(activeSearch, true)}
        >
          加载更多
        </button>
      )}
      {listBusy && (
        <p role="status" className={style.notice}>
          正在读取候选窗皮肤…
        </p>
      )}
      {publishOpen && localSkins && (
        <CandidateSkinPublishDialog
          client={client}
          localSkins={localSkins}
          openSkinDirectory={openSkinDirectory}
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
