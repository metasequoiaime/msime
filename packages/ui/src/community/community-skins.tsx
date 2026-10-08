// Source: MSIME-Apple@9ca823ab40018ced3cb71812503dbc3b94615ac0
// (`SkinCommunityView.swift`, `CommunityGalleryStyle.swift`).
import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { ScreenKeyboardPreview } from "../keyboard/screen-keyboard-preview";
import {
  communitySkinMessage,
  communitySkinPublishMessage,
  communityNeedsSignIn,
  communityPublishLoginAction,
} from "./community-helpers";
import { useCommunityGallery, type CommunityGalleryClient } from "./community-gallery";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunityDetailStatus } from "./community-detail-status";
import { StatusMessage } from "../core/status-message";
import { CommunityDetailFrame } from "./community-detail-frame";
import { CommunityDetailHeader } from "./community-detail-header";
import * as style from "./community-style";
import { CommunitySearchForm, type CommunitySearchFormProps } from "./community-search-form";
import { CommunityScopeButtons } from "./community-scope-buttons";
import {
  CommunityReportSection,
  type CommunityModeration,
  type CommunityReportReason,
} from "./community-report";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import { CommunityNotice } from "./community-notice";
import { CommunitySelectField } from "./community-select-field";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityCardMetrics } from "./community-card-metrics";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCard } from "./community-card";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { CommunityGalleryHeading } from "./community-gallery-heading";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { CommunityLoadMoreButton } from "./community-gallery-controls";
import { ActionButton } from "../core/action-button";
import { useToast } from "../core/toast";
import { deepEqual } from "../core/deep-equal";
import { boundedGraphemes } from "../core/text";
import { updateCustomKeyboard } from "../settings/theme-selection-updates";
import type { Preferences } from "../index";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";
import { useCommunityDetailHistory } from "./use-community-detail-history";
import { communityPublishFields } from "./community-publish-validation";
import {
  CommunitySkinCategoryFilter,
  CommunitySkinCategorySelect,
  communitySkinCategories,
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
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // Kept next to the sentence because publishMessage collapses the code, and this is the one
  // failure the dialog can do something about rather than only name.
  const [signInRequired, setSignInRequired] = useState(false);
  const {
    busy: actionBusy,
    generation: clientGeneration,
    running: actionRunning,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, undefined, client, library);
  const busy = loading || actionBusy;

  useEffect(() => {
    const generation = clientGeneration.current;
    setLoading(true);
    void library
      .load()
      .then((items) => {
        if (generation !== clientGeneration.current) return;
        setSaved(items);
        const first = items[0];
        if (first) {
          setSelectedId(first.id);
          setName(first.name);
        }
      })
      .catch((loadError) => {
        if (generation !== clientGeneration.current) return;
        setError(communitySkinPublishMessage(loadError));
        setSignInRequired(communityNeedsSignIn(loadError));
      })
      .finally(() => {
        if (generation === clientGeneration.current) setLoading(false);
      });
  }, [client, clientGeneration, library]);

  const selected = saved.find((item) => item.id === selectedId) ?? null;
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || actionRunning.current || !selected) return;
    const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
      communityPublishFields(name, description);
    if (!nameValid || !descriptionValid || !agreed) {
      setError("请填写有效名称和说明，并确认拥有公开发布所需的素材权利。");
      return;
    }
    setSignInRequired(false);
    await runAsyncAction(
      async (isCurrent) => {
        await client.publish(
          publicationId,
          normalizedName,
          normalizedDescription,
          selected.design,
          category,
        );
        if (!isCurrent()) return;
        await onPublished();
      },
      {
        formatError: communitySkinPublishMessage,
        onError: (publishError) => setSignInRequired(communityNeedsSignIn(publishError)),
      },
    );
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
      {busy && saved.length === 0 && <StatusMessage role="status">正在读取我的皮肤…</StatusMessage>}
      {!busy && saved.length === 0 && (
        <CommunityNotice>还没有命名保存的皮肤，请先在“设计我的皮肤”中保存一款。</CommunityNotice>
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
          <CommunityPublicationWarning>
            发布后设计及照片壁纸将公开。请勿包含私人照片或敏感信息；发布成功后可在“我的作品”中下架。
          </CommunityPublicationWarning>
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
    <CommunityCard aria-label={`查看皮肤 ${skin.name}`} onClick={open}>
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
    </CommunityCard>
  );
}

/** HarmonyOS 手机的搜索：一个胶囊输入框，按 Enter 提交，没有单独的搜索按钮。 */
export function CommunityHarmonySearch({
  label,
  value,
  onChange,
  onSubmit,
}: CommunitySearchFormProps) {
  return (
    <form
      className="m-0 min-w-0"
      role="search"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      <input
        className={style.harmonySearch}
        aria-label={label}
        placeholder={label}
        enterKeyHint="search"
        value={value}
        onChange={(event) => onChange(boundedGraphemes(event.target.value, 128))}
      />
    </form>
  );
}

/** HarmonyOS 手机的「加载更多」：列表下方一个低调的文字按钮，下一页加载期间显示加载中的提示行。 */
export function CommunityHarmonyLoadMore({
  hasMore,
  busy,
  loadingText,
  onLoadMore,
}: {
  hasMore: boolean;
  busy: boolean;
  loadingText: string;
  onLoadMore: () => void;
}) {
  if (!hasMore && !busy) return null;
  return (
    <>
      {hasMore && (
        <CommunityLoadMoreButton
          className={style.harmonyMore}
          disabled={busy}
          onClick={onLoadMore}
        />
      )}
      {busy && (
        <StatusMessage role="status" className={style.harmonyNotice}>
          {loadingText}
        </StatusMessage>
      )}
    </>
  );
}

/** 皮肤库条目按 id 与社区列表项对应，而 id 是 UUID，大小写都可能出现。 */
function skinKey(id: string): string {
  return id.toLowerCase();
}

/** 皮肤卡片上胶囊按钮提供的操作：把设计收进皮肤库，把库里已有的设计用到键盘上，或者因为已在使用而不提供操作。 */
type CommunitySkinPillState = "get" | "getting" | "use" | "using" | "in-use";

const skinPillLabels: Record<CommunitySkinPillState, string> = {
  get: "获取",
  getting: "获取中",
  use: "使用",
  using: "使用",
  "in-use": "使用中",
};

/** HarmonyOS 手机上的皮肤卡片：预览图、名称、作者和使用次数，以及一个「获取 / 使用 / 使用中」胶囊按钮。点胶囊以外的任何位置打开详情，评分、举报和试用仍在详情里。 */
function CommunityHarmonySkinCard({
  skin,
  theme,
  pill,
  open,
  act,
}: {
  skin: CommunitySkin;
  theme: "light" | "dark";
  pill: CommunitySkinPillState;
  open: () => void;
  act: () => void;
}) {
  const author = [
    communitySkinCategoryLabel(skin.category),
    skin.owned ? "我的作品" : skin.author,
    skin.owned && skin.moderation === "removed" ? "已下架" : "",
  ]
    .filter(Boolean)
    .join(" · ");
  const settled = pill === "in-use";
  return (
    // 卡片本身是打开详情的指针目标；底部文字是可获得焦点的按钮，为键盘和读屏用户做同样的事，它的点击和轻点一样传到卡片上。
    <div className={style.harmonySkinCard} onClick={open}>
      <span className={style.harmonySkinStage}>
        <span className={style.harmonySkinStageClip}>
          <ScreenKeyboardPreview theme={theme} skin="custom" customDesign={skin.design} thumbnail />
        </span>
      </span>
      <div className={style.harmonySkinFooter}>
        <button
          type="button"
          className={style.harmonySkinText}
          aria-label={`查看皮肤 ${skin.name}`}
        >
          <span className={style.harmonySkinName}>{skin.name}</span>
          {author && <span className={style.harmonySkinMeta}>{author}</span>}
          <CommunityCardMetrics
            look="harmony"
            downloads={skin.downloads}
            ratingCount={skin.rating_count}
            ratingAverage={skin.rating_average}
          />
        </button>
        <button
          type="button"
          className={`${style.harmonySkinPill} ${settled ? style.harmonyPillDone : style.harmonyPillTonal}`}
          aria-label={`${skinPillLabels[pill]}皮肤 ${skin.name}`}
          disabled={pill !== "get" && pill !== "use"}
          onClick={(event) => {
            // 胶囊按钮直接作用于卡片的皮肤，不打开详情。
            event.stopPropagation();
            act();
          }}
        >
          {skinPillLabels[pill]}
        </button>
      </div>
    </div>
  );
}

export function CommunitySkinsPage({
  client,
  theme,
  localSkinLibrary,
  initialMine = false,
  mobile = false,
  onLogin,
  look,
  preferences,
  onApplyPreferences,
  onSkinApplied,
}: {
  client: CommunitySkinClient;
  theme: "light" | "dark";
  localSkinLibrary?: CustomSkinLibraryClient;
  initialMine?: boolean;
  mobile?: boolean;
  /** Where the account page is, for a publish that failed only because nobody is signed in. */
  onLogin?: () => void;
  /** HarmonyOS 手机的「社区」标签页：胶囊搜索框、一行筛选标签、带「获取 / 使用」胶囊按钮的双列卡片。其他宿主不设置它，保持原来的社区列表。 */
  look?: "harmony";
  /** 正在编辑的偏好，用来判断库里的某个设计是否正用在键盘上（使用中）。 */
  preferences?: Preferences;
  /** 应用一项偏好修改；「使用」和在皮肤页选设计一样经由它生效。 */
  onApplyPreferences?: (next: Preferences) => void | Promise<void>;
  /** 「使用」刚把某个皮肤用到键盘上时，以该皮肤的社区 id 调用。 */
  onSkinApplied?: (id: string) => void;
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
    replaceSelected,
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

  const harmony = look === "harmony";
  const toast = useToast();
  // 按小写 id 索引的自定义皮肤库。「获取」保存的皮肤以其社区 id 归档，卡片据此改为提供「使用」。
  const [library, setLibrary] = useState<ReadonlyMap<string, SavedTouchKeyboardSkin>>(
    () => new Map(),
  );
  const [pending, setPending] = useState<ReadonlyMap<string, "getting" | "using">>(() => new Map());
  // 同步读取，这样第一次点击尚未渲染时再点一次胶囊按钮不会有任何效果。
  const acting = useRef(new Set<string>());
  // 每次「获取」完成都加一，这样更早开始的皮肤库读取不会丢掉它刚加入的设计。
  const libraryGeneration = useRef(0);
  const takingTrials = useRef(new Map<string, CommunitySkinTrial>());
  useEffect(() => {
    const trials = takingTrials.current;
    return () => {
      for (const unfinished of trials.values()) {
        void client.finishTrial(unfinished.id, false).catch(() => undefined);
      }
      trials.clear();
    };
  }, [client]);
  // 网格重新显示时就重新读取：详情里的「下载并试用」也会把设计收进皮肤库。
  const browsing = selected === null;
  useEffect(() => {
    if (!harmony || !browsing || !localSkinLibrary) return;
    let current = true;
    const generation = libraryGeneration.current;
    void localSkinLibrary.load().then(
      (items) => {
        if (!current || generation !== libraryGeneration.current) return;
        setLibrary(new Map(items.map((item) => [skinKey(item.id), item])));
      },
      // 皮肤库读不出来时所有卡片都停在「获取」，与 Android 一致；再次获取同一设计会按同一 id 重新归档，不会多出一份。
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [harmony, browsing, localSkinLibrary]);

  const closeDetail = async (fromHistory = false) => {
    if (actionBusy) return;
    if (trial) {
      let finished = false;
      await runAction(async (generation) => {
        await client.finishTrial(trial.id, false);
        if (!gallery.isCurrent(generation)) return;
        trialRef.current = null;
        setTrial(null);
        finished = true;
      });
      if (!finished) return;
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

  useCommunityDetailHistory({
    mobile,
    kind: "skin",
    selectedId: selected?.id ?? null,
    onClose: () => void closeDetail(true),
  });

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
        replaceSelected(updated);
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
    if (harmony) toast("已发布到社区。");
    await requestList(activeSearch, false);
  };

  const pillState = (skin: CommunitySkin): CommunitySkinPillState => {
    const key = skinKey(skin.id);
    const busy = pending.get(key);
    if (busy) return busy;
    const saved = library.get(key);
    if (!saved) return "get";
    return preferences?.global_theme === "custom" &&
      deepEqual(preferences.custom_theme?.keyboard, saved.design)
      ? "in-use"
      : "use";
  };

  const setPendingState = (key: string, state: "getting" | "using" | null) =>
    setPending((current) => {
      const next = new Map(current);
      if (state) next.set(key, state);
      else next.delete(key);
      return next;
    });

  /**
   * 获取：把设计保存进自定义皮肤库，不改变键盘，与 Android 的胶囊按钮一致。
   *
   * 宿主只有一个下载操作，而它做的不止这些：取回设计、用它开始试用，并以社区 id 导入皮肤库，这样服务器会计入这次下载，之后「使用」也能按这个 id 找到设计。结束试用但不保留时会换回之前的皮肤，皮肤库条目保持不动。没能结束的试用留在 `takingTrials` 里，下面的清理逻辑会在页面离开时重试。
   */
  const take = async (skin: CommunitySkin) => {
    const key = skinKey(skin.id);
    if (acting.current.has(key)) return;
    acting.current.add(key);
    setPendingState(key, "getting");
    try {
      const result = await client.download(skin.id, skin.name);
      takingTrials.current.set(key, result.trial);
      await client.finishTrial(result.trial.id, false);
      takingTrials.current.delete(key);
      libraryGeneration.current += 1;
      setLibrary((current) => new Map(current).set(skinKey(result.skin.id), result.skin));
      toast(`已获取「${skin.name}」，点「使用」换上`);
    } catch (failure) {
      // `communitySkinMessage` 会写明皮肤库最多十二个设计的上限，这是用户能自己处理的失败。
      toast(communitySkinMessage(failure));
    } finally {
      acting.current.delete(key);
      setPendingState(key, null);
    }
  };

  /** 使用：把库里的副本用到键盘上，与在皮肤页选择已保存的设计一样，经由同一项偏好修改。 */
  const apply = async (skin: CommunitySkin) => {
    const key = skinKey(skin.id);
    const saved = library.get(key);
    if (!saved || !preferences || !onApplyPreferences || acting.current.has(key)) return;
    acting.current.add(key);
    setPendingState(key, "using");
    try {
      await onApplyPreferences(updateCustomKeyboard(preferences, saved.design));
      onSkinApplied?.(skin.id);
      toast(`已换上「${skin.name}」`);
    } catch {
      toast("切换失败，保留当前皮肤");
    } finally {
      acting.current.delete(key);
      setPendingState(key, null);
    }
  };

  if (selected)
    return (
      <CommunityDetailFrame
        backDisabled={actionBusy}
        onBack={() => void closeDetail()}
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
      >
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
      </CommunityDetailFrame>
    );

  const publishDialog = publishOpen && localSkinLibrary && (
    <CommunitySkinPublishDialog
      client={client}
      library={localSkinLibrary}
      onClose={() => setPublishOpen(false)}
      onPublished={publishDone}
      onLogin={communityPublishLoginAction(() => setPublishOpen(false), onLogin)}
    />
  );

  if (harmony) {
    const shown = skins.filter((skin) => !mineOnly || skin.owned);
    return (
      <div className={style.harmonyPage}>
        <CommunityHarmonySearch
          label="搜索皮肤设计"
          value={search}
          onChange={setSearch}
          onSubmit={() => void requestList(search, false)}
        />
        {/* 设计去掉了桌面版的标题。分类标签在一行内滚动，范围切换和「发布」以纯文字留在这一行末尾，仍可点到。 */}
        <div className={style.harmonyChips}>
          <div className="contents" role="group" aria-label="键盘皮肤分类">
            <button
              type="button"
              className={style.harmonyChip}
              aria-pressed={categoryFilter.category === null}
              onClick={() => void changeCategory(null)}
            >
              全部
            </button>
            {communitySkinCategories.map((item) => (
              <button
                key={item}
                type="button"
                className={style.harmonyChip}
                aria-pressed={categoryFilter.category === item}
                onClick={() => void changeCategory(item)}
              >
                {communitySkinCategoryLabels[item]}
              </button>
            ))}
          </div>
          <span className={style.harmonyChipDivider} aria-hidden="true" />
          <button
            type="button"
            className={style.harmonyChipAction}
            aria-pressed={mineOnly}
            onClick={() => {
              const nextMineOnly = !mineOnly;
              setMineOnly(nextMineOnly);
              void requestList(activeSearch, false, nextMineOnly);
            }}
          >
            我的作品
          </button>
          {localSkinLibrary && (
            <button
              type="button"
              className={style.harmonyChipAction}
              onClick={() => setPublishOpen(true)}
            >
              发布设计
            </button>
          )}
        </div>
        <CommunityGalleryFeedback
          error={error}
          signInRequired={signInRequired}
          onLogin={onLogin}
          empty={
            !listBusy && shown.length === 0 ? (
              <p className={style.harmonyNotice}>
                {mineOnly
                  ? hasMore
                    ? "当前页没有你的作品，请继续加载查看更多。"
                    : "还没有已发布的皮肤。"
                  : "暂时没有匹配的皮肤。"}
              </p>
            ) : undefined
          }
        />
        {shown.length > 0 && (
          <div className={style.harmonyGrid}>
            {shown.map((skin) => {
              const pill = pillState(skin);
              return (
                <CommunityHarmonySkinCard
                  key={skin.id}
                  skin={skin}
                  theme={theme}
                  pill={pill}
                  open={() => open(skin)}
                  act={() => void (pill === "use" ? apply(skin) : take(skin))}
                />
              );
            })}
          </div>
        )}
        <CommunityHarmonyLoadMore
          hasMore={hasMore}
          busy={listBusy}
          loadingText="正在读取社区皮肤…"
          onLoadMore={() => void requestList(activeSearch, true)}
        />
        {publishDialog}
      </div>
    );
  }

  return (
    <CommunityPageShell>
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
      <CommunityGalleryFeedback
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
        empty={
          !listBusy && skins.filter((skin) => !mineOnly || skin.owned).length === 0 ? (
            <CommunityNotice>
              {mineOnly
                ? hasMore
                  ? "当前页没有你的作品，请继续加载查看更多。"
                  : "还没有已发布的皮肤。"
                : "暂时没有匹配的皮肤。"}
            </CommunityNotice>
          ) : undefined
        }
      />
      <CommunityGalleryGrid>
        {skins
          .filter((skin) => !mineOnly || skin.owned)
          .map((skin) => (
            <CommunitySkinCard key={skin.id} skin={skin} theme={theme} open={() => open(skin)} />
          ))}
      </CommunityGalleryGrid>
      <CommunityGalleryLoadMore
        hasMore={hasMore}
        busy={listBusy}
        loadingText="正在读取社区皮肤…"
        onLoadMore={() => void requestList(activeSearch, true)}
      />
      {publishDialog}
    </CommunityPageShell>
  );
}
