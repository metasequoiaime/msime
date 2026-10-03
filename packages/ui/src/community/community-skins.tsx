// Source: MSIME-Apple@9ca823ab40018ced3cb71812503dbc3b94615ac0
// (`SkinCommunityView.swift`, `CommunityGalleryStyle.swift`).
import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { boundedGraphemes } from "../core/text";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import {
  communitySkinMessage,
  communitySkinPublishMessage,
  communityNeedsSignIn,
  runCommunityPublishAction,
} from "./community-helpers";
import { useCommunityGallery, type CommunityGalleryClient } from "./community-gallery";
import { CommunityErrorAlert } from "./community-error-alert";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunityDetailStatus } from "./community-detail-status";
import { CommunityDetailHeader } from "./community-detail-header";
import * as style from "./community-style";
import { CommunitySearchForm } from "./community-search-form";
import { CommunityScopeButtons } from "./community-scope-buttons";
import {
  CommunityReportSection,
  type CommunityModeration,
  type CommunityReportReason,
} from "./community-report";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunitySelectField } from "./community-select-field";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityCardMetrics } from "./community-card-metrics";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityBackButton } from "./community-gallery-controls";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryHeading } from "./community-gallery-heading";
import { ActionButton } from "../core/action-button";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";
import {
  CommunitySkinCategoryFilter,
  CommunitySkinCategorySelect,
  communitySkinCategoryLabel,
  communitySkinCategoryLabels,
  useCommunitySkinCategoryFilter,
  type CommunitySkinCategory,
} from "./community-skin-category";
import type {
  CustomSkinLibraryClient,
  SavedTouchKeyboardSkin,
  TouchKeyboardSkinDesign,
} from "../keyboard/touch-keyboard-skin-design";

export type CommunitySkin = {
  id: string;
  name: string;
  description: string;
  author: string;
  design: TouchKeyboardSkinDesign;
  downloads: number;
  rating_count: number;
  rating_average: number;
  owned: boolean;
  my_rating: number;
  /** Sent only on the user's own skins; `removed` shows 已下架. */
  moderation?: CommunityModeration | null;
  /** 发布分类；早于分类功能的服务端不返回。 */
  category?: CommunitySkinCategory;
};

export type CommunitySkinPage = {
  skins: CommunitySkin[];
  has_more: boolean;
};

export type CommunitySkinTrial = { id: string; name: string };
export type CommunitySkinDownload = {
  skin: { id: string; name: string; design: TouchKeyboardSkinDesign };
  trial: CommunitySkinTrial;
};

export interface CommunitySkinClient {
  /** `mine` lists only the signed-in user's own skins, removed ones included. `category` 为 `null` 时列出全部分类。 */
  list(
    offset: number,
    search: string,
    mine: boolean,
    category: CommunitySkinCategory | null,
  ): Promise<CommunitySkinPage>;
  detail(id: string): Promise<CommunitySkin>;
  download(id: string, name: string): Promise<CommunitySkinDownload>;
  rate(id: string, stars: number): Promise<void>;
  publish(
    id: string,
    name: string,
    description: string,
    design: TouchKeyboardSkinDesign,
    category: CommunitySkinCategory,
  ): Promise<void>;
  unpublish(id: string): Promise<void>;
  /** 修改自己作品的发布分类，返回修改后的条目。 */
  setCategory(id: string, category: CommunitySkinCategory): Promise<CommunitySkin>;
  finishTrial(id: string, keep: boolean): Promise<void>;
  /** Reports another user's skin to the moderators. */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}

function CommunitySkinPublishDialog({
  client,
  library,
  onClose,
  onPublished,
  onLogin,
}: {
  client: CommunitySkinClient;
  library: CustomSkinLibraryClient;
  onClose: () => void;
  onPublished: () => Promise<void>;
  /** Where to send someone who has to sign in before publishing; absent leaves the sentence alone. */
  onLogin?: () => void;
}) {
  const [saved, setSaved] = useState<SavedTouchKeyboardSkin[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const {
    name,
    description,
    agreed,
    publicationId,
    setName,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    resetPublication,
  } = useCommunityPublicationDraft();
  const [category, setCategory] = useState<CommunitySkinCategory>("other");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  // Kept next to the sentence because publishMessage collapses the code, and this is the one
  // failure the dialog can do something about rather than only name.
  const [signInRequired, setSignInRequired] = useState(false);
  const clientGeneration = useRef(0);
  const actionRunning = useRef(false);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    let active = true;
    actionRunning.current = false;
    setBusy(true);
    void library
      .load()
      .then((items) => {
        if (!active) return;
        setSaved(items);
        const first = items[0];
        if (first) {
          setSelectedId(first.id);
          setName(first.name);
        }
      })
      .catch((loadError) => {
        if (!active) return;
        setError(communitySkinPublishMessage(loadError));
        setSignInRequired(communityNeedsSignIn(loadError));
      })
      .finally(() => {
        if (active) setBusy(false);
      });
    return () => {
      active = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client, library]);

  const selected = saved.find((item) => item.id === selectedId) ?? null;
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || actionRunning.current || !selected) return;
    const generation = clientGeneration.current;
    const normalizedName = name.trim();
    const normalizedDescription = description.trim();
    if (
      !normalizedName ||
      boundedGraphemes(normalizedName, 32) !== normalizedName ||
      [...normalizedName].length > 32 ||
      [...normalizedDescription].length > 280 ||
      !agreed
    ) {
      setError("请填写有效名称和说明，并确认拥有公开发布所需的素材权利。");
      return;
    }
    await runCommunityPublishAction({
      busy,
      generation,
      clientGeneration,
      actionRunning,
      setBusy,
      setError,
      setSignInRequired,
      formatError: communitySkinPublishMessage,
      operation: async () => {
        await client.publish(
          publicationId,
          normalizedName,
          normalizedDescription,
          selected.design,
          category,
        );
        if (generation !== clientGeneration.current) return;
        await onPublished();
      },
    });
  };

  return (
    <CommunityDialogFrame
      title="发布我的皮肤"
      ariaLabel="发布我的皮肤"
      busy={busy}
      onClose={onClose}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
      onSubmit={(event) => void submit(event)}
    >
      {busy && saved.length === 0 && <p role="status">正在读取我的皮肤…</p>}
      {!busy && saved.length === 0 && (
        <p className={style.notice}>还没有命名保存的皮肤，请先在“设计我的皮肤”中保存一款。</p>
      )}
      {saved.length > 0 && (
        <>
          <CommunitySelectField
            label="发布设计"
            ariaLabel="发布设计"
            value={selectedId}
            disabled={busy}
            onChange={(nextId) => {
              const item = saved.find((value) => value.id === nextId);
              setSelectedId(nextId);
              resetPublication();
              if (item) setName(item.name);
            }}
          >
            {saved.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </CommunitySelectField>
          {selected && (
            <div className={`${style.cardStage} max-h-[220px]`}>
              <ScreenKeyboardPreview theme="light" skin="custom" customDesign={selected.design} />
            </div>
          )}
          <CommunitySkinPublicationFields
            name={name}
            description={description}
            agreed={agreed}
            busy={busy}
            agreementText="我拥有发布所用素材的权利，并同意其他用户免费下载使用"
            onNameChange={onNameChange}
            onDescriptionChange={onDescriptionChange}
            onAgreedChange={onAgreedChange}
          />
          <CommunitySkinCategorySelect
            ariaLabel="发布分类"
            value={category}
            disabled={busy}
            onChange={(next) => {
              // 分类也是这次发布的内容，换了分类就是另一次发布，不能沿用上一次的发布 id。
              resetPublication();
              setCategory(next);
            }}
          />
          <p className={style.warning}>
            发布后设计及照片壁纸将公开。请勿包含私人照片或敏感信息；发布成功后可在“我的作品”中下架。
          </p>
        </>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        <button type="submit" className="primary" disabled={busy || !selected || !agreed}>
          {busy ? "正在发布…" : "公开发布"}
        </button>
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}

function CommunitySkinCard({
  skin,
  theme,
  open,
}: {
  skin: CommunitySkin;
  theme: "light" | "dark";
  open: () => void;
}) {
  return (
    <button
      type="button"
      className={style.card}
      aria-label={`查看皮肤 ${skin.name}`}
      onClick={open}
    >
      <span className={style.cardStage}>
        <ScreenKeyboardPreview theme={theme} skin="custom" customDesign={skin.design} compact />
      </span>
      <strong>{skin.name}</strong>
      <CommunityCardAuthor
        prefix={communitySkinCategoryLabel(skin.category) ?? undefined}
        author={skin.author}
        owned={skin.owned}
        removed={skin.moderation === "removed"}
      />
      <CommunityCardMetrics
        downloads={skin.downloads}
        ratingCount={skin.rating_count}
        ratingAverage={skin.rating_average}
      />
    </button>
  );
}

export function CommunitySkinsPage({
  client,
  theme,
  localSkinLibrary,
  initialMine = false,
  mobile = false,
  onLogin,
}: {
  client: CommunitySkinClient;
  theme: "light" | "dark";
  localSkinLibrary?: CustomSkinLibraryClient;
  initialMine?: boolean;
  mobile?: boolean;
  /** Where the account page is, for a publish that failed only because nobody is signed in. */
  onLogin?: () => void;
}) {
  const categoryFilter = useCommunitySkinCategoryFilter();
  const categoryRequest = categoryFilter.request;
  const galleryClient = useMemo<CommunityGalleryClient<CommunitySkin>>(
    () => ({
      list: async (offset, search, mine) => {
        const page = await client.list(offset, search, mine ?? false, categoryRequest.current);
        return { items: page.skins, has_more: page.has_more };
      },
      detail: client.detail,
      rate: client.rate,
      unpublish: client.unpublish,
      ...(client.report && {
        report: (id: string, reason: CommunityReportReason, detail: string) =>
          client.report!(id, reason, detail),
      }),
    }),
    [client, categoryRequest],
  );
  const gallery = useCommunityGallery({
    client: galleryClient,
    initialMine,
    errorMessage: communitySkinMessage,
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
    setSelected,
    setActionNotice,
    setMineOnly,
    setConfirmUnpublish,
    requestList,
    open: openGallery,
    closeDetail: closeGalleryDetail,
    rateSelected,
    unpublishSelected,
    reportSelected,
    runAction,
  } = gallery;
  const [search, setSearch] = useState("");
  const [trial, setTrial] = useState<CommunitySkinTrial | null>(null);
  const [publishOpen, setPublishOpen] = useState(false);
  const trialRef = useRef<CommunitySkinTrial | null>(null);
  useEffect(() => {
    return () => {
      const pending = trialRef.current;
      trialRef.current = null;
      setTrial(null);
      if (pending) void client.finishTrial(pending.id, false).catch(() => undefined);
    };
  }, [client]);

  const closeDetail = async (fromHistory = false) => {
    if (actionBusy) return;
    if (trial) {
      const generation = gallery.beginAction();
      if (generation === null) return;
      try {
        await client.finishTrial(trial.id, false);
        if (!gallery.isCurrent(generation)) return;
        trialRef.current = null;
        setTrial(null);
      } catch (actionError) {
        gallery.fail(actionError);
        gallery.endAction(generation);
        return;
      }
      gallery.endAction(generation);
    }
    closeGalleryDetail();
    if (
      !fromHistory &&
      mobile &&
      typeof window !== "undefined" &&
      window.history.state?.communityDetail?.kind === "skin"
    ) {
      window.history.back();
    }
  };

  const open = (skin: CommunitySkin) => {
    if (mobile && typeof window !== "undefined") {
      pushMobileSettingsState({
        page: "community",
        communityDetail: { kind: "skin", id: skin.id },
      });
    }
    openGallery(skin);
  };

  useEffect(() => {
    if (!mobile || typeof window === "undefined") return;
    const onPopState = (event: PopStateEvent) => {
      const detail = event.state?.communityDetail;
      if (selected && !(detail?.kind === "skin" && detail.id === selected.id))
        void closeDetail(true);
    };
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, [mobile, selected, closeDetail]);

  const download = async () => {
    if (!selected) return;
    await runAction(
      async (currentClient) => {
        const result = await client.download(selected.id, selected.name);
        if (!gallery.isCurrent(currentClient)) return;
        setSelected((current) => (current ? { ...current, design: result.skin.design } : current));
        trialRef.current = result.trial;
        setTrial(result.trial);
        setActionNotice("已下载并开始试用；关闭此页会恢复原皮肤。");
      },
      { clearNotice: true },
    );
  };

  const finishTrial = async (keep: boolean) => {
    if (!trial) return;
    const pending = trial;
    await runAction(async (currentClient) => {
      await client.finishTrial(pending.id, keep);
      if (!gallery.isCurrent(currentClient)) return;
      trialRef.current = null;
      setTrial(null);
      setActionNotice(keep ? "已保留这款皮肤。" : "已恢复试用前的皮肤。");
    });
  };

  const changeCategory = (next: CommunitySkinCategory | null) =>
    categoryFilter.change(next, () => requestList(activeSearch, false));

  const changeOwnCategory = async (next: CommunitySkinCategory) => {
    if (!selected) return;
    const target = selected;
    await runAction(
      async (currentClient) => {
        const updated = await client.setCategory(target.id, next);
        if (!gallery.isCurrent(currentClient)) return;
        setSelected(updated);
        gallery.setItems((current) =>
          current.map((item) => (item.id === updated.id ? updated : item)),
        );
        setActionNotice(`已改为「${communitySkinCategoryLabels[next]}」分类。`);
      },
      { clearNotice: true },
    );
  };

  const unpublish = () => {
    if (trial) return;
    void unpublishSelected("已下架这款皮肤；其他用户将无法再下载，已有本地副本不会受影响。");
  };

  const publishDone = async () => {
    setPublishOpen(false);
    setActionNotice("已发布到社区。");
    await requestList(activeSearch, false);
  };

  if (selected)
    return (
      <div className={style.page}>
        <CommunityBackButton disabled={actionBusy} onClick={() => void closeDetail()} />
        {error && (
          <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
        )}
        <section className={`section ${style.detail}`}>
          <div className={style.detailStage}>
            <ScreenKeyboardPreview theme={theme} skin="custom" customDesign={selected.design} />
          </div>
          <CommunityDetailHeader
            title={selected.name}
            note={[communitySkinCategoryLabel(selected.category), selected.author]
              .filter(Boolean)
              .join(" · ")}
            owned={selected.owned}
            moderation={selected.moderation}
            description={selected.description}
          />
          <CommunityDetailStatus
            downloads={selected.downloads}
            ratingCount={selected.rating_count}
            ratingAverage={selected.rating_average}
            myRating={selected.my_rating}
            detailBusy={detailBusy}
            actionNotice={actionNotice}
            loadingText="正在读取皮肤详情…"
          />
          {!trial && (
            <ActionButton
              action={() => void download()}
              className={`primary ${style.action}`}
              disabled={actionBusy || detailBusy}
              label="下载并试用"
            />
          )}
          {trial && (
            <div
              className={`${style.divided} grid grid-cols-2 gap-2 [&>p]:col-span-full [&>p]:mt-0 [&>p]:mb-2.5 [&>p]:text-xs [&>p]:text-secondary`}
              aria-label="皮肤试用"
            >
              <p>正在试用：{trial.name}</p>
              <ActionButton
                action={() => void finishTrial(false)}
                className="secondary"
                disabled={actionBusy}
                label="恢复原皮肤"
              />
              <ActionButton
                action={() => void finishTrial(true)}
                className="primary"
                disabled={actionBusy}
                label="保留使用"
              />
            </div>
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
            actionBusy={actionBusy}
            ratingDescription="我的评分（下载后可评，可重新选择）"
            unpublishMessage={`下架后其他用户无法再下载，已下载的本地皮肤会保留。确定下架“${selected.name}”吗？`}
            unpublishDisabled={Boolean(trial)}
            confirmUnpublish={confirmUnpublish}
            onRate={(stars) => void rateSelected(stars)}
            onRequestUnpublish={() => setConfirmUnpublish(true)}
            onUnpublish={() => void unpublish()}
            onCancelUnpublish={() => setConfirmUnpublish(false)}
          />
          {!selected.owned && client.report && (
            <CommunityReportSection actionBusy={actionBusy} onReport={reportSelected} />
          )}
        </section>
      </div>
    );

  return (
    <div className={style.page}>
      <CommunitySearchForm
        label="搜索皮肤设计"
        value={search}
        onChange={setSearch}
        onSubmit={() => void requestList(search, false)}
      />
      <CommunityGalleryHeading
        title={mineOnly ? "你的公开设计" : "换个心情，从键盘开始"}
        note={mineOnly ? "管理你发布到社区的皮肤" : "发现创作者的配色与巧思，找到你的那一款"}
      >
        <CommunityScopeButtons
          ariaLabel="社区皮肤范围"
          mineOnly={mineOnly}
          allLabel="全部皮肤"
          mineLabel="我的作品"
          onMineOnlyChange={(nextMineOnly) => {
            setMineOnly(nextMineOnly);
            void requestList(activeSearch, false, nextMineOnly);
          }}
        />
        {localSkinLibrary && (
          <ActionButton
            action={() => setPublishOpen(true)}
            className="primary"
            label="发布我的设计"
          />
        )}
      </CommunityGalleryHeading>
      <CommunitySkinCategoryFilter
        ariaLabel="键盘皮肤分类"
        value={categoryFilter.category}
        onChange={(next) => void changeCategory(next)}
      />
      {error && (
        <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
      )}
      {!listBusy && skins.filter((skin) => !mineOnly || skin.owned).length === 0 && (
        <p className={style.notice}>
          {mineOnly
            ? hasMore
              ? "当前页没有你的作品，请继续加载查看更多。"
              : "还没有已发布的皮肤。"
            : "暂时没有匹配的皮肤。"}
        </p>
      )}
      <div className={style.grid}>
        {skins
          .filter((skin) => !mineOnly || skin.owned)
          .map((skin) => (
            <CommunitySkinCard key={skin.id} skin={skin} theme={theme} open={() => open(skin)} />
          ))}
      </div>
      <CommunityGalleryLoadMore
        hasMore={hasMore}
        busy={listBusy}
        loadingText="正在读取社区皮肤…"
        onLoadMore={() => void requestList(activeSearch, true)}
      />
      {publishOpen && localSkinLibrary && (
        <CommunitySkinPublishDialog
          client={client}
          library={localSkinLibrary}
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
