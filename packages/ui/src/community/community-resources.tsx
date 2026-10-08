import { useEffect, useRef, useState, type FormEvent } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { randomUuid } from "../core/random-id";
import { formatZhNumber } from "../core/format-number";
import { pushMobileSettingsState } from "../settings/mobile-navigation";
import {
  CommunityHarmonyLoadMore,
  CommunityHarmonySearch,
  CommunitySkinsPage,
  type CommunitySkinClient,
} from "./community-skins";
import { useToast } from "../core/toast";
import { boundedGraphemes } from "../core/text";
import type { Preferences } from "../index";
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

/** 宿主能否把词条导入本机词库。 */
function importsLocally(
  dictionary: CommunityLocalDictionaryClient | undefined,
): dictionary is Required<CommunityLocalDictionaryClient> {
  return Boolean(dictionary?.import);
}

/**
 * 把共享词库的词条导入本机词库，每种词条类型发一次请求，返回实际应用的词条数。详情页的「导入这版词库到本机」和 HarmonyOS 行上的「添加」都走这里。
 */
async function importCommunityDictionary(
  dictionary: Required<CommunityLocalDictionaryClient>,
  entries: readonly CommunitySharedWord[],
): Promise<number> {
  const groups = new Map<CommunitySharedWord["kind"], CommunitySharedWord[]>();
  for (const entry of entries) {
    const group = groups.get(entry.kind) ?? [];
    group.push(entry);
    groups.set(entry.kind, group);
  }
  let applied = 0;
  for (const [entryKind, group] of groups) {
    const kind = entryKind === "quick" ? "quick_phrase" : entryKind;
    const text = group.map((entry) => `${entry.word}\t${entry.code}\t${entry.weight}`).join("\n");
    const result = await dictionary.import(
      kind,
      "standard",
      text,
      `community-local-${Date.now()}-${entryKind}`,
    );
    applied += result.applied;
  }
  return applied;
}

/**
 * HarmonyOS 手机上行下方的元信息行，对应 Android 的 `CommunityRequest.resourceSubtitle`：「@作者 · 4,812 条」。只写条目本身带有的信息：没有作者时不写作者，只有带了词条的词库才写条数，用户自己已下架的作品写「已下架」。设计稿中的「本周更新」需要更新时间，而本客户端的资源不带这个字段，所以从不写出。
 */
export function communityResourceSubtitle(item: CommunityResource): string {
  const entries = item.content.entries;
  return [
    item.author ? `@${item.author}` : "",
    item.kind === "dictionary" && entries && entries.length > 0
      ? `${formatZhNumber(entries.length)} 条`
      : "",
    item.owned && item.moderation === "removed" ? "已下架" : "",
  ]
    .filter(Boolean)
    .join(" · ");
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
      if (!importsLocally(localDictionary)) return;
      const applied = await importCommunityDictionary(localDictionary, item.content.entries ?? []);
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

/** 行上的胶囊按钮提供什么：添加该条目、添加进行中，或因本次会话已添加而不提供。 */
type CommunityResourcePillState = "add" | "adding" | "added";

const resourcePillLabels: Record<CommunityResourcePillState, string> = {
  add: "添加",
  adding: "添加中",
  added: "已添加",
};

/** HarmonyOS 手机上的一行词库或回复模板：色调底块上的首字、名称、元信息行和一个「添加」胶囊按钮。点按胶囊以外的任何地方都会打开详情。 */
function CommunityHarmonyResourceRow({
  item,
  pill,
  open,
  add,
}: {
  item: CommunityResource;
  pill: CommunityResourcePillState;
  open: () => void;
  add: () => void;
}) {
  const subtitle = communityResourceSubtitle(item);
  return (
    // 整行是打开详情的指针目标；行内文字是可聚焦的按钮，为键盘和读屏用户做同样的事，该按钮的点击会像轻点一样传到整行。
    <div className={style.harmonyRow} onClick={open}>
      <span className={style.harmonyRowTile} aria-hidden="true">
        {boundedGraphemes(item.name.trim(), 1) || "?"}
      </span>
      <button
        type="button"
        className={style.harmonyRowText}
        aria-label={`查看${resourceKindTitle(item.kind)} ${item.name}`}
      >
        <span className={style.harmonyRowName}>{item.name}</span>
        {subtitle && <span className={style.harmonyRowMeta}>{subtitle}</span>}
        {item.kind === "reply" && item.description && (
          <span className={style.harmonyRowDescription}>{item.description}</span>
        )}
      </button>
      <button
        type="button"
        className={`${style.harmonyRowPill} ${pill === "added" ? style.harmonyPillDone : style.harmonyPillTonal}`}
        aria-label={`${resourcePillLabels[pill]}${resourceKindTitle(item.kind)} ${item.name}`}
        disabled={pill !== "add"}
        onClick={(event) => {
          // 胶囊按钮作用于本行的条目，不打开其详情。
          event.stopPropagation();
          add();
        }}
      >
        {resourcePillLabels[pill]}
      </button>
    </div>
  );
}

export function CommunityResourcesPage({
  client,
  kind,
  initialScope = "",
  localDictionary,
  mobile = false,
  look,
}: {
  client: CommunityResourceClient;
  kind: CommunityResourceKind;
  initialScope?: CommunityResourceScope;
  localDictionary?: CommunityLocalDictionaryClient;
  mobile?: boolean;
  /** HarmonyOS 手机的「社区」页：胶囊搜索框、范围筛选 chip，以及一张由多行组成、每行带「添加」胶囊按钮的分组卡片。其他宿主不设置。 */
  look?: "harmony";
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
  const toast = useToast();
  // 本次会话已添加和正在添加的条目，按 id 记录。没有宿主会报告哪些共享词库或回复模板已在设备上，所以每行初始都是「添加」；再次添加只会导入相同的词条或保留同一个模板，不产生任何变化。
  const [added, setAdded] = useState<ReadonlySet<string>>(() => new Set());
  const [adding, setAdding] = useState<ReadonlySet<string>>(() => new Set());
  const addingNow = useRef(new Set<string>());
  /** 「添加」：与详情页主按钮同一路径，之后弹出 toast。宿主能本地导入时，词库进入本机词库，不能时进入云词库；回复模板保存给「高情商回复」键盘使用。 */
  const add = async (item: CommunityResource) => {
    if (addingNow.current.has(item.id)) return;
    addingNow.current.add(item.id);
    setAdding((current) => new Set(current).add(item.id));
    try {
      if (item.kind === "dictionary") {
        // 列表里的副本可能比服务器将导入的版本旧，所以词条取自重新获取的详情，与详情页的做法相同。
        const latest = await client.detail(item.id);
        if (importsLocally(localDictionary)) {
          const applied = await importCommunityDictionary(
            localDictionary,
            latest.content.entries ?? [],
          );
          toast(`已添加「${item.name}」，本机词库应用 ${applied} 个词条`);
        } else {
          const result = await client.apply(latest.id, latest.revision);
          toast(`已添加「${item.name}」，云端词库新增或更新 ${result.imported} 个词条`);
        }
      } else {
        await client.save(item.id, true);
        const latest = await client.detail(item.id);
        await client.storeReply(latest);
        toast(`已添加「${item.name}」到高情商回复键盘`);
      }
      setAdded((current) => new Set(current).add(item.id));
    } catch (failure) {
      toast(resourceMessage(failure));
    } finally {
      addingNow.current.delete(item.id);
      setAdding((current) => {
        const next = new Set(current);
        next.delete(item.id);
        return next;
      });
    }
  };
  if (selected)
    return (
      <ResourceDetail
        client={client}
        initial={selected}
        close={closeDetail}
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
        <CommunityHarmonySearch
          label={`搜索${resourceKindTitle(kind)}`}
          value={search}
          onChange={setSearch}
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
                onClick={() => setScope(option.scope)}
              >
                {option.label}
              </button>
            ))}
          </div>
          <span className={style.harmonyChipDivider} aria-hidden="true" />
          <button
            type="button"
            className={style.harmonyChipAction}
            onClick={() => setEditing(true)}
          >
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
        <CommunityHarmonyLoadMore
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
      </div>
    );
  }
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
  look,
  preferences,
  onApplyPreferences,
  onSkinApplied,
}: {
  skins: CommunitySkinClient;
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
  const [category, setCategory] = useState<"skin" | CommunityResourceKind>(initialCategory);
  const harmony = look === "harmony";
  // 设计稿把第三段命名为「短语」。本宿主没有接入短语包，所以在 HarmonyOS 上这一段放的是回复模板，使用它们自己的分区标题。
  const tabs: { category: "skin" | CommunityResourceKind; label: string }[] = [
    { category: "skin", label: "皮肤" },
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
      {category === "skin" ? (
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
          kind={category}
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
