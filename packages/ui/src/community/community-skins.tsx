// Source: MSIME-Apple@9ca823ab40018ced3cb71812503dbc3b94615ac0
// (`SkinCommunityView.swift`, `CommunityGalleryStyle.swift`).
import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { boundedGraphemes } from "../core/text";
import { randomUuid } from "../core/random-id";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import {
  communityNeedsSignIn,
  communityRating,
  communitySkinMessage,
  communitySkinPublishMessage,
} from "./community-helpers";
import { useCommunityGallery, type CommunityGalleryClient } from "./community-gallery";
import * as style from "./community-style";
import { CommunitySearchForm } from "./community-search-form";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunitySkinModerationSection } from "./community-skin-moderation-section";
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
  list(offset: number, search: string): Promise<CommunitySkinPage>;
  detail(id: string): Promise<CommunitySkin>;
  download(id: string, name: string): Promise<CommunitySkinDownload>;
  rate(id: string, stars: number): Promise<void>;
  publish(
    id: string,
    name: string,
    description: string,
    design: TouchKeyboardSkinDesign,
  ): Promise<void>;
  unpublish(id: string): Promise<void>;
  finishTrial(id: string, keep: boolean): Promise<void>;
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
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [agreed, setAgreed] = useState(false);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  // Kept next to the sentence because publishMessage collapses the code, and this is the one
  // failure the dialog can do something about rather than only name.
  const [signInRequired, setSignInRequired] = useState(false);
  const [publicationId, setPublicationId] = useState(randomUuid);
  const clientGeneration = useRef(0);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    let active = true;
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
    if (busy || !selected) return;
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
    setBusy(true);
    setError("");
    setSignInRequired(false);
    try {
      await client.publish(publicationId, normalizedName, normalizedDescription, selected.design);
      if (generation !== clientGeneration.current) return;
      await onPublished();
    } catch (publishError) {
      if (generation !== clientGeneration.current) return;
      setError(communitySkinPublishMessage(publishError));
      setSignInRequired(communityNeedsSignIn(publishError));
      if (generation === clientGeneration.current) setBusy(false);
    }
  };

  return (
    <div className={style.backdrop}>
      <form
        className={style.dialog}
        role="dialog"
        aria-modal="true"
        aria-label="发布我的皮肤"
        onSubmit={(event) => void submit(event)}
      >
        <div className={style.dialogHeading}>
          <h2>发布我的皮肤</h2>
          <button
            type="button"
            className={style.dialogClose}
            disabled={busy}
            onClick={onClose}
            aria-label="关闭发布窗口"
          >
            ×
          </button>
        </div>
        {error && (
          <p role="alert" className="error">
            {error}
            {/* The source opens the sign-in form in place rather than telling the user to go and
                find it, which from a modal is the difference between one tap and four. */}
            {signInRequired && onLogin && (
              <>
                {" "}
                <button type="button" className="secondary" onClick={onLogin}>
                  去登录
                </button>
              </>
            )}
          </p>
        )}
        {busy && saved.length === 0 && <p role="status">正在读取我的皮肤…</p>}
        {!busy && saved.length === 0 && (
          <p className={style.notice}>还没有命名保存的皮肤，请先在“设计我的皮肤”中保存一款。</p>
        )}
        {saved.length > 0 && (
          <>
            <label className={style.field}>
              发布设计
              <select
                className={style.fieldControl}
                aria-label="发布设计"
                value={selectedId}
                disabled={busy}
                onChange={(event) => {
                  const item = saved.find((value) => value.id === event.target.value);
                  setSelectedId(event.target.value);
                  setPublicationId(randomUuid());
                  if (item) setName(item.name);
                }}
              >
                {saved.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name}
                  </option>
                ))}
              </select>
            </label>
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
              onNameChange={(value) => {
                setPublicationId(randomUuid());
                setName(boundedGraphemes(value, 32));
              }}
              onDescriptionChange={(value) => {
                setPublicationId(randomUuid());
                setDescription(value);
              }}
              onAgreedChange={setAgreed}
            />
            <p className={style.warning}>
              发布后设计及照片壁纸将公开。请勿包含私人照片或敏感信息；发布成功后可在“我的作品”中下架。
            </p>
          </>
        )}
        <div className={style.dialogActions}>
          <button type="button" className="secondary" disabled={busy} onClick={onClose}>
            取消
          </button>
          <button type="submit" className="primary" disabled={busy || !selected || !agreed}>
            {busy ? "正在发布…" : "公开发布"}
          </button>
        </div>
      </form>
    </div>
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
      <span className={style.cardAuthor}>{skin.owned ? "我的作品" : skin.author}</span>
      <span className={style.cardMetrics}>
        <span>↓ {skin.downloads.toLocaleString("zh-CN")}</span>
        <span>☆ {communityRating(skin.rating_count, skin.rating_average)}</span>
      </span>
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
  const galleryClient = useMemo<CommunityGalleryClient<CommunitySkin>>(
    () => ({
      list: async (offset, search) => {
        const page = await client.list(offset, search);
        return { items: page.skins, has_more: page.has_more };
      },
      detail: client.detail,
      rate: client.rate,
      unpublish: client.unpublish,
    }),
    [client],
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
  } = gallery;
  const [search, setSearch] = useState("");
  const [trial, setTrial] = useState<CommunitySkinTrial | null>(null);
  const [publishOpen, setPublishOpen] = useState(false);
  const trialRef = useRef<CommunitySkinTrial | null>(null);
  useEffect(() => {
    return () => {
      const pending = trialRef.current;
      trialRef.current = null;
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
    const currentClient = gallery.beginAction();
    if (currentClient === null) return;
    setActionNotice("");
    try {
      const result = await client.download(selected.id, selected.name);
      if (!gallery.isCurrent(currentClient)) return;
      setSelected((current) => (current ? { ...current, design: result.skin.design } : current));
      trialRef.current = result.trial;
      setTrial(result.trial);
      setActionNotice("已下载并开始试用；关闭此页会恢复原皮肤。");
    } catch (actionError) {
      if (gallery.isCurrent(currentClient)) {
        gallery.fail(actionError);
      }
    } finally {
      gallery.endAction(currentClient);
    }
  };

  const finishTrial = async (keep: boolean) => {
    if (!trial) return;
    const currentClient = gallery.beginAction();
    if (currentClient === null) return;
    try {
      await client.finishTrial(trial.id, keep);
      if (!gallery.isCurrent(currentClient)) return;
      trialRef.current = null;
      setTrial(null);
      setActionNotice(keep ? "已保留这款皮肤。" : "已恢复试用前的皮肤。");
    } catch (actionError) {
      if (gallery.isCurrent(currentClient)) gallery.fail(actionError);
    } finally {
      gallery.endAction(currentClient);
    }
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
        <button
          type="button"
          className={style.back}
          disabled={actionBusy}
          onClick={() => void closeDetail()}
          aria-label="返回社区"
        >
          ← 社区
        </button>
        {error && (
          <p role="alert" className="error">
            {error}
            {/* The detail view is where a signed-out download fails, so the way out belongs here
                too rather than only on the gallery behind it. */}
            {signInRequired && onLogin && (
              <>
                {" "}
                <button type="button" className="secondary" onClick={onLogin}>
                  去登录
                </button>
              </>
            )}
          </p>
        )}
        <section className={`section ${style.detail}`}>
          <div className={style.detailStage}>
            <ScreenKeyboardPreview theme={theme} skin="custom" customDesign={selected.design} />
          </div>
          <div className={style.detailTitle}>
            <div className={style.headingBody}>
              <h2 className={style.headingTitle}>{selected.name}</h2>
              <p className={style.headingNote}>{selected.author}</p>
            </div>
            {selected.owned && <span className={style.detailBadge}>我的作品</span>}
          </div>
          {selected.description && <p className={style.description}>{selected.description}</p>}
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
          {!trial && (
            <button
              type="button"
              className={`primary ${style.action}`}
              disabled={actionBusy || detailBusy}
              onClick={() => void download()}
            >
              下载并试用
            </button>
          )}
          {trial && (
            <div
              className={`${style.divided} grid grid-cols-2 gap-2 [&>p]:col-span-full [&>p]:mt-0 [&>p]:mb-2.5 [&>p]:text-xs [&>p]:text-secondary`}
              aria-label="皮肤试用"
            >
              <p>正在试用：{trial.name}</p>
              <button
                type="button"
                className="secondary"
                disabled={actionBusy}
                onClick={() => void finishTrial(false)}
              >
                恢复原皮肤
              </button>
              <button
                type="button"
                className="primary"
                disabled={actionBusy}
                onClick={() => void finishTrial(true)}
              >
                保留使用
              </button>
            </div>
          )}
          <CommunitySkinModerationSection
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
      <div className={style.heading}>
        <div className={style.headingBody}>
          <h2 className={style.headingTitle}>
            {mineOnly ? "你的公开设计" : "换个心情，从键盘开始"}
          </h2>
          <p className={style.headingNote}>
            {mineOnly ? "管理你发布到社区的皮肤" : "发现创作者的配色与巧思，找到你的那一款"}
          </p>
        </div>
        <div className={style.headingActions}>
          <CommunityScopeButtons
            ariaLabel="社区皮肤范围"
            mineOnly={mineOnly}
            allLabel="全部皮肤"
            mineLabel="我的作品"
            onMineOnlyChange={(nextMineOnly) => {
              setMineOnly(nextMineOnly);
              void requestList(activeSearch, false);
            }}
          />
          {localSkinLibrary && (
            <button type="button" className="primary" onClick={() => setPublishOpen(true)}>
              发布我的设计
            </button>
          )}
        </div>
      </div>
      {error && (
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
          正在读取社区皮肤…
        </p>
      )}
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
