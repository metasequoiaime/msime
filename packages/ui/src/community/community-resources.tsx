import { useEffect, useRef, useState, type FormEvent } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { randomUuid } from "../core/random-id";
import { formatZhNumber } from "../core/format-number";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import { CommunitySkinsPage, type CommunitySkinClient } from "./community-skins";
import {
  appendUniqueById,
  communityRating,
  resourceKindTitle,
  resourceMessage,
} from "./community-helpers";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import * as style from "./community-style";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCard } from "./community-card";
import { CommunityMetrics } from "./community-metrics";
import { CommunitySearchForm } from "./community-search-form";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunityDetailHeader } from "./community-detail-header";
import { CommunityRatingMetrics } from "./community-rating-metrics";
import {
  CommunityReportSection,
  communityReportedNotice,
  type CommunityModeration,
  type CommunityReportReason,
} from "./community-report";
import {
  CommunityResourceScopeButtons,
  type CommunityResourceScope,
} from "./community-resource-scope-buttons";
import { CommunityInputField } from "./community-input-field";
import { CommunitySelectField } from "./community-select-field";
import { CommunityTextareaField } from "./community-textarea-field";
import { CommunityPublicationMetadataFields } from "./community-publication-metadata-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import { CommunityNotice } from "./community-notice";
import { CommunityDetailFrame } from "./community-detail-frame";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { ActionButton } from "../core/action-button";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityGalleryHeading } from "./community-gallery-heading";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { communityPublishFields } from "./community-publish-validation";
import { useCommunityClientLifecycle } from "./use-community-client-lifecycle";
import { useCommunityDetailHistory } from "./use-community-detail-history";

export type CommunityResourceKind = "dictionary" | "reply";
export type { CommunityResourceScope } from "./community-resource-scope-buttons";
export type CommunitySharedWord = {
  kind: "pinyin" | "wubi" | "quick" | "english";
  code: string;
  word: string;
  weight: number;
};
export type CommunityResourceContent = { entries?: CommunitySharedWord[]; prompt?: string };
export type CommunityResource = {
  id: string;
  kind: CommunityResourceKind;
  name: string;
  description: string;
  author: string;
  content: CommunityResourceContent;
  revision: number;
  saves: number;
  saved: boolean;
  owned: boolean;
  rating_count: number;
  rating_average: number;
  my_rating: number;
  /** Sent only on the user's own items; `removed` shows 已下架. */
  moderation?: CommunityModeration | null;
};
export type CommunityResourcePage = { items: CommunityResource[]; has_more: boolean };
export type CommunityResourceApplication = {
  revision: number;
  imported: number;
  resource_revision: number;
};
export type CommunityLocalDictionaryClient = {
  import?(
    kind: "pinyin" | "wubi" | "quick_phrase" | "english",
    format: "standard",
    text: string,
    requestId: string,
  ): Promise<{ applied: number }>;
};

export interface CommunityResourceClient {
  list(
    kind: CommunityResourceKind,
    scope: CommunityResourceScope,
    search: string,
    offset: number,
  ): Promise<CommunityResourcePage>;
  detail(id: string): Promise<CommunityResource>;
  publish(
    id: string,
    kind: CommunityResourceKind,
    name: string,
    description: string,
    content: CommunityResourceContent,
    revision: number,
  ): Promise<void>;
  apply(id: string, resourceRevision: number): Promise<CommunityResourceApplication>;
  save(id: string, saved: boolean): Promise<void>;
  rate(id: string, stars: number): Promise<void>;
  unpublish(id: string): Promise<void>;
  storeReply(item: CommunityResource): Promise<void>;
  removeReply(id: string): Promise<void>;
  /** Reports another user's dictionary or reply template to the moderators. */
  report?(
    kind: CommunityResourceKind,
    id: string,
    reason: CommunityReportReason,
    detail: string,
  ): Promise<void>;
}

function ResourceCard({ item, open }: { item: CommunityResource; open: () => void }) {
  return (
    <CommunityCard onClick={open} aria-label={`查看${resourceKindTitle(item.kind)} ${item.name}`}>
      <span className={style.resourceIcon} aria-hidden="true">
        {item.kind === "dictionary" ? "字" : "话"}
      </span>
      <strong className={style.cardTitle}>{item.name}</strong>
      <CommunityCardAuthor
        author={item.author}
        owned={item.owned}
        removed={item.moderation === "removed"}
      />
      <span className={style.resourceDescription}>
        {item.description || (item.kind === "dictionary" ? "共享词条" : "回复模板")}
      </span>
      <span className={style.cardMetrics}>
        ☆ {communityRating(item.rating_count, item.rating_average)} · {formatZhNumber(item.saves)}{" "}
        人收藏
      </span>
    </CommunityCard>
  );
}

function ResourceEditor({
  client,
  kind,
  existing,
  close,
  onPublished,
}: {
  client: CommunityResourceClient;
  kind: CommunityResourceKind;
  existing?: CommunityResource;
  close: () => void;
  onPublished: () => Promise<void>;
}) {
  const [id] = useState(existing?.id ?? randomUuid());
  const [name, setName] = useState(existing?.name ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [prompt, setPrompt] = useState(existing?.content.prompt ?? "");
  const [entries, setEntries] = useState<CommunitySharedWord[]>(existing?.content.entries ?? []);
  const [entryKind, setEntryKind] = useState<CommunitySharedWord["kind"]>("pinyin");
  const [code, setCode] = useState("");
  const [word, setWord] = useState("");
  const [weight, setWeight] = useState("100000");
  const [agreed, setAgreed] = useState(false);
  const [error, setError] = useState("");
  const {
    busy,
    running: actionRunning,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, undefined, client);
  const addEntry = () => {
    const value = { kind: entryKind, code: code.trim(), word, weight: Number(weight) };
    if (
      !value.code ||
      !value.word ||
      !Number.isSafeInteger(value.weight) ||
      value.weight < 0 ||
      entries.some(
        (item) => item.kind === value.kind && item.code === value.code && item.word === value.word,
      ) ||
      entries.length >= 128
    ) {
      setError("词条不能为空、不能重复，权重必须为非负整数，且最多 128 条。");
      return;
    }
    setEntries((current) => [...current, value]);
    setCode("");
    setWord("");
    setError("");
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy || actionRunning.current) return;
    const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
      communityPublishFields(name, description);
    const valid =
      nameValid &&
      descriptionValid &&
      (kind === "reply"
        ? prompt.trim().length > 0 && [...prompt].length <= 2000
        : entries.length > 0) &&
      (existing || agreed);
    if (!valid) {
      setError(
        existing ? "请填写有效的作品信息。" : "请填写有效内容并确认拥有公开发布所需的权利。",
      );
      return;
    }
    await runAsyncAction(
      async (isCurrent) => {
        await client.publish(
          id,
          kind,
          normalizedName,
          normalizedDescription,
          kind === "reply" ? { prompt } : { entries },
          existing?.revision ?? 0,
        );
        if (!isCurrent()) return;
        await onPublished();
      },
      { formatError: resourceMessage },
    );
  };
  return (
    <CommunityDialogFrame
      title={existing ? "更新作品" : `发布${resourceKindTitle(kind)}`}
      titleClassName={style.dialogTitle}
      ariaLabel={existing ? "更新社区作品" : `发布${resourceKindTitle(kind)}`}
      busy={busy}
      onClose={close}
      error={error}
      onSubmit={(event) => void submit(event)}
    >
      <CommunityPublicationMetadataFields
        name={name}
        description={description}
        agreed={agreed}
        busy={busy}
        nameLabel="作品名称"
        nameAriaLabel="社区作品名称"
        descriptionLabel="作品说明"
        descriptionAriaLabel="社区作品说明"
        descriptionRows={3}
        showAgreement={!existing}
        agreementText="我拥有发布所用内容的权利，并同意其他用户查看和使用"
        onNameChange={setName}
        onDescriptionChange={setDescription}
        onAgreedChange={setAgreed}
      />
      {kind === "reply" ? (
        <CommunityTextareaField
          label="回复提示词"
          ariaLabel="社区回复提示词"
          maxLength={2000}
          rows={8}
          value={prompt}
          disabled={busy}
          onChange={setPrompt}
        />
      ) : (
        <>
          <div className={style.entryForm}>
            <CommunitySelectField
              label="类型"
              ariaLabel="社区词条类型"
              value={entryKind}
              disabled={busy}
              onChange={(value) => setEntryKind(value as CommunitySharedWord["kind"])}
            >
              <option value="pinyin">拼音</option>
              <option value="wubi">五笔</option>
              <option value="quick">快捷短语</option>
              <option value="english">英文</option>
            </CommunitySelectField>
            <CommunityInputField
              label="编码"
              ariaLabel="社区词条编码"
              value={code}
              disabled={busy}
              onChange={setCode}
            />
            <CommunityInputField
              label="词语"
              ariaLabel="社区词条文字"
              value={word}
              disabled={busy}
              onChange={setWord}
            />
            <CommunityInputField
              label="权重"
              ariaLabel="社区词条权重"
              type="number"
              value={weight}
              disabled={busy}
              onChange={setWeight}
            />
            <ActionButton
              action={addEntry}
              className="secondary"
              disabled={busy}
              label="添加词条"
            />
          </div>
          <div className={style.entryList} aria-label={`待发布词条 ${entries.length}/128`}>
            {entries.map((item, index) => (
              <div key={`${item.kind}-${item.code}-${item.word}-${index}`}>
                <span>
                  {item.word} · <code>{item.code}</code> · {item.weight}
                </span>
                <ActionButton
                  action={() => setEntries(entries.filter((_, current) => current !== index))}
                  className="secondary"
                  disabled={busy}
                  label="移除"
                />
              </div>
            ))}
          </div>
        </>
      )}
      <CommunityPublicationWarning>
        发布内容会公开展示。请勿包含 API
        Key、私人聊天内容或其他个人资料；发布后可在“我的作品”中下架。
      </CommunityPublicationWarning>
      <CommunityDialogActions busy={busy} onClose={close}>
        <button type="submit" className="primary" disabled={busy}>
          {busy ? "正在发布…" : existing ? "发布新版本" : "公开发布"}
        </button>
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}

function ResourceDetail({
  client,
  initial,
  close,
  localDictionary,
}: {
  client: CommunityResourceClient;
  initial: CommunityResource;
  close: () => void;
  localDictionary?: CommunityLocalDictionaryClient;
}) {
  const [item, setItem] = useState(initial);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const {
    busy,
    mounted,
    generation: clientGeneration,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, setNotice, client, initial.id);
  const renderGeneration = clientGeneration.current;
  const run = (action: (generation: number) => Promise<void>) => {
    const generation = clientGeneration.current;
    return runAsyncAction(() => action(generation), {
      formatError: resourceMessage,
    });
  };
  useEffect(() => {
    const generation = clientGeneration.current;
    void client
      .detail(initial.id)
      .then((value) => {
        if (mounted.current && generation === clientGeneration.current) setItem(value);
      })
      .catch((errorValue) => {
        if (mounted.current && generation === clientGeneration.current)
          setError(resourceMessage(errorValue));
      });
  }, [client, initial.id, clientGeneration, mounted]);
  const save = () =>
    void run(async (generation) => {
      await client.save(item.id, !item.saved);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const updated = await client.detail(item.id);
      if (mounted.current && generation === clientGeneration.current) setItem(updated);
    });
  const apply = () =>
    void run(async (generation) => {
      const result = await client.apply(item.id, item.revision);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice(`已导入云端词库，新增或更新 ${result.imported} 个词条。`);
    });
  const applyLocal = () =>
    void run(async (generation) => {
      if (!localDictionary?.import) return;
      const groups = new Map<CommunitySharedWord["kind"], CommunitySharedWord[]>();
      for (const entry of item.content.entries ?? []) {
        const group = groups.get(entry.kind) ?? [];
        group.push(entry);
        groups.set(entry.kind, group);
      }
      let applied = 0;
      for (const [entryKind, entries] of groups) {
        const kind = entryKind === "quick" ? "quick_phrase" : entryKind;
        const text = entries
          .map((entry) => `${entry.word}\t${entry.code}\t${entry.weight}`)
          .join("\n");
        const result = await localDictionary.import(
          kind,
          "standard",
          text,
          `community-local-${Date.now()}-${entryKind}`,
        );
        applied += result.applied;
      }
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice(`已导入本机词库，应用 ${applied} 个词条。`);
    });
  const storeReply = () =>
    void run(async (generation) => {
      await client.save(item.id, true);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const latest = await client.detail(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      await client.storeReply(latest);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setItem(latest);
      setNotice("已添加到高情商回复键盘；只有点按生成时才会发送文字。");
    });
  const rateResource = (stars: number) =>
    void run(async (generation) => {
      await client.rate(item.id, stars);
      if (!mounted.current || generation !== clientGeneration.current) return;
      const updated = await client.detail(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setItem(updated);
      setNotice(`已评分：${stars} 星。`);
    });
  const removeReply = () =>
    void run(async (generation) => {
      await client.removeReply(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setNotice("已从本机高情商回复键盘移除，社区收藏保留。");
    });
  const unpublish = () =>
    void run(async (generation) => {
      await client.unpublish(item.id);
      if (!mounted.current || generation !== clientGeneration.current) return;
      setConfirmDelete(false);
      close();
    });
  const report = async (reason: CommunityReportReason, detail: string) => {
    if (!client.report) return false;
    let reported = false;
    await run(async (generation) => {
      await client.report!(item.kind, item.id, reason, detail);
      if (!mounted.current || generation !== clientGeneration.current) return;
      reported = true;
      setNotice(communityReportedNotice);
    });
    return reported;
  };
  return (
    <CommunityDetailFrame backDisabled={busy} onBack={close} backAriaLabel="← 社区" error={error}>
      <CommunityDetailHeader
        title={item.name}
        note={`${item.author} · v${item.revision}`}
        owned={item.owned}
        moderation={item.moderation}
        description={item.description}
      />
      <CommunityRatingMetrics
        count={item.saves}
        countLabel="收藏"
        ratingCount={item.rating_count}
        ratingAverage={item.rating_average}
      />
      {item.kind === "dictionary" ? (
        <>
          <h3>词条预览 · {(item.content.entries ?? []).length} 条</h3>
          <div className={style.entryPreview}>
            {(item.content.entries ?? []).map((entry, index) => (
              <div key={`${entry.kind}-${entry.code}-${index}`}>
                <span>{entry.word}</span>
                <code>{entry.code}</code>
              </div>
            ))}
          </div>
          {localDictionary?.import && (
            <ActionButton
              action={applyLocal}
              className={`primary ${style.action}`}
              disabled={busy}
              label="导入这版词库到本机"
            />
          )}
          <ActionButton
            action={apply}
            className={`secondary ${style.action}`}
            disabled={busy}
            label="导入这版词库到云端"
          />
          <CommunityMetrics className={style.divided}>
            本机导入只更新当前设备；云端导入会合并到账号云词库。版本发生变化时云端导入会停止并要求重新查看。
          </CommunityMetrics>
        </>
      ) : (
        <>
          <h3>提示词预览</h3>
          <pre className={style.promptPreview}>{item.content.prompt}</pre>
          <ActionButton
            action={storeReply}
            className={`primary ${style.action}`}
            disabled={busy}
            label="添加到高情商回复键盘"
          />
          <ActionButton
            action={removeReply}
            className={`secondary ${style.action}`}
            disabled={busy}
            label="从本机高情商回复键盘移除"
          />
        </>
      )}
      {notice && <CommunityActionNotice>{notice}</CommunityActionNotice>}
      <ActionButton
        action={save}
        className={`secondary ${style.action}`}
        disabled={busy}
        label={item.saved ? "取消收藏" : "收藏，关注后续更新"}
      />
      {item.owned && (
        <ActionButton
          action={() => setEditing(true)}
          className={`secondary ${style.action}`}
          disabled={busy}
          label="编辑并发布新版本"
        />
      )}
      <CommunityModerationSection
        owned={item.owned}
        actionBusy={busy}
        ratingDescription="我的评分（可重新选择）"
        unpublishMessage={
          <>
            下架后其他用户无法获取此作品，已有本地回复模板和云词库副本不会被删除。确定下架“
            {item.name}”吗？
          </>
        }
        confirmUnpublish={confirmDelete}
        onRate={rateResource}
        onRequestUnpublish={() => setConfirmDelete(true)}
        onUnpublish={unpublish}
        onCancelUnpublish={() => setConfirmDelete(false)}
        unpublishButtonClassName="danger-text community-unpublish"
        unpublishLabel="下架作品"
        unpublishConfirmLabel="确认下架作品"
      />
      {!item.owned && client.report && (
        <CommunityReportSection actionBusy={busy} onReport={report} />
      )}
      {editing && (
        <ResourceEditor
          client={client}
          kind={item.kind}
          existing={item}
          close={() => setEditing(false)}
          onPublished={async () => {
            setEditing(false);
            const updated = await client.detail(item.id);
            if (!mounted.current || renderGeneration !== clientGeneration.current) return;
            setItem(updated);
          }}
        />
      )}
    </CommunityDetailFrame>
  );
}

export function CommunityResourcesPage({
  client,
  kind,
  initialScope = "",
  localDictionary,
  mobile = false,
}: {
  client: CommunityResourceClient;
  kind: CommunityResourceKind;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  mobile?: boolean;
}) {
  const [scope, setScope] = useState<CommunityResourceScope>(initialScope);
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<CommunityResource[]>([]);
  const [more, setMore] = useState(false);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<CommunityResource | null>(null);
  const [editing, setEditing] = useState(false);
  const { mounted, clientGeneration } = useCommunityClientLifecycle(client, kind, scope);
  const activeSearch = useRef("");
  const load = async (append = false, query = activeSearch.current) => {
    const current = ++clientGeneration.current;
    setBusy(true);
    setError("");
    const offset = append ? items.length : 0;
    try {
      const page = await client.list(kind, scope, query, offset);
      if (!mounted.current || current !== clientGeneration.current) return;
      setItems((value) => (append ? appendUniqueById(value, page.items) : page.items));
      setMore(page.has_more);
      if (!append) activeSearch.current = query;
    } catch (loadError) {
      if (mounted.current && current === clientGeneration.current) {
        setError(resourceMessage(loadError));
        // The existing rows belong to the previous query or scope. Do not let
        // their continuation offset be used with the failed fresh request.
        if (!append) setMore(false);
      }
    } finally {
      if (mounted.current && current === clientGeneration.current) setBusy(false);
    }
  };
  useEffect(() => {
    void load();
  }, [client, kind, scope]);
  const openDetail = (item: CommunityResource) => {
    if (mobile && typeof window !== "undefined") {
      pushMobileSettingsState({ page: "community", communityDetail: { kind, id: item.id } });
    }
    setSelected(item);
  };
  const closeDetail = () => {
    if (
      mobile &&
      typeof window !== "undefined" &&
      window.history.state?.communityDetail?.kind === kind
    ) {
      window.history.back();
      return;
    }
    setSelected(null);
  };
  useCommunityDetailHistory({
    mobile,
    kind,
    selectedId: selected?.id ?? null,
    onClose: () => setSelected(null),
  });
  if (selected)
    return (
      <ResourceDetail
        client={client}
        initial={selected}
        close={closeDetail}
        localDictionary={localDictionary}
      />
    );
  return (
    <CommunityPageShell>
      <CommunitySearchForm
        label={`搜索${resourceKindTitle(kind)}`}
        value={search}
        onChange={setSearch}
        onSubmit={() => void load(false, search)}
      />
      <CommunityGalleryHeading
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
          onScopeChange={setScope}
        />
        <ActionButton action={() => setEditing(true)} className="primary" label="发布作品" />
      </CommunityGalleryHeading>
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
          <ResourceCard key={item.id} item={item} open={() => openDetail(item)} />
        ))}
      </CommunityGalleryGrid>
      <CommunityGalleryLoadMore
        hasMore={more}
        busy={busy}
        loadingText="正在读取社区…"
        onLoadMore={() => void load(true)}
      />
      {editing && (
        <ResourceEditor
          client={client}
          kind={kind}
          close={() => setEditing(false)}
          onPublished={async () => {
            setEditing(false);
            await load();
          }}
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
}: {
  skins: CommunitySkinClient;
  resources: CommunityResourceClient;
  theme: "light" | "dark";
  initialMine?: boolean;
  initialCategory?: "skin" | CommunityResourceKind;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  /**
   * The saved designs the skin gallery publishes from.
   *
   * It was absent here, and this page is what a host with both skins and resources renders, so on
   * exactly those hosts — HarmonyOS and Android — 发布我的设计 was never drawn and the publish flow
   * had no entry point at all. The desktop path renders CommunitySkinsPage directly and was fine.
   */
  localSkinLibrary?: CustomSkinLibraryClient;
  mobile?: boolean;
  onLogin?: () => void;
}) {
  const [category, setCategory] = useState<"skin" | CommunityResourceKind>(initialCategory);
  return (
    <CommunityPageShell>
      <div className={style.categoryTabs} role="tablist" aria-label="社区分类">
        <button
          type="button"
          role="tab"
          aria-selected={category === "skin"}
          onClick={() => setCategory("skin")}
        >
          皮肤
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={category === "dictionary"}
          onClick={() => setCategory("dictionary")}
        >
          词库
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={category === "reply"}
          onClick={() => setCategory("reply")}
        >
          回复模板
        </button>
      </div>
      {category === "skin" ? (
        <CommunitySkinsPage
          client={skins}
          theme={theme}
          initialMine={initialMine}
          localSkinLibrary={localSkinLibrary}
          mobile={mobile}
          onLogin={onLogin}
        />
      ) : (
        <CommunityResourcesPage
          client={resources}
          kind={category}
          initialScope={initialScope}
          localDictionary={localDictionary}
          mobile={mobile}
        />
      )}
    </CommunityPageShell>
  );
}
