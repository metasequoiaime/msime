import { useEffect, useMemo, useRef, useState } from "react";
import { boundedGraphemes } from "../core/text";
import {
  communityPublishFields,
  handleCommunityPublishKeyDown,
} from "./community-publish-validation";
import {
  kindLabels,
  type PluginCatalogResult,
  type PluginKind,
  type PluginPackage,
} from "../settings/plugins-section";
import { packKindLabel } from "../settings/plugin-catalog-helpers";
import {
  candidateSkinMegabytes,
  communityNeedsSignIn,
  communityPluginMessage,
  communityPublishLoginAction,
  runCommunityPublishAction,
} from "./community-helpers";
import { useCommunityGallery, type CommunityGalleryClient } from "./community-gallery";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunityDetailStatus } from "./community-detail-status";
import { CommunityDetailHeader } from "./community-detail-header";
import * as style from "./community-style";
import { CommunitySearchForm } from "./community-search-form";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunitySelectField } from "./community-select-field";
import { CommunityPublicationMetadataFields } from "./community-publication-metadata-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import {
  CommunityReportSection,
  type CommunityModeration,
  type CommunityReportReason,
} from "./community-report";
import { CommunityCardMetrics } from "./community-card-metrics";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityInstallButton } from "./community-install-button";
import { CommunityReplaceConfirmation } from "./community-replace-confirmation";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { CommunityDetailFrame } from "./community-detail-frame";
import { ActionButton } from "../core/action-button";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityGalleryHeading } from "./community-gallery-heading";

/** The kinds a pack can be shared as; effect packs stay local for now. Mirrors `client-core::plugins::community::PUBLISHABLE_KINDS`. */
export type CommunityPluginKind = Exclude<PluginKind, "effect">;

export const communityPluginKinds: readonly CommunityPluginKind[] = [
  "sound",
  "music",
  "command_table",
  "phrase_table",
  "helpcode",
  "wordbook",
  "symbol_set",
];

/** One publication, `client-core::plugins::community::CommunityPlugin`. */
export type CommunityPlugin = {
  id: string;
  kind: CommunityPluginKind;
  /** The manifest id, which is also the folder the pack installs into. */
  plugin_id: string;
  name: string;
  description: string;
  author: string;
  version: string;
  license: string;
  size: number;
  sha256: string;
  downloads: number;
  rating_count: number;
  rating_average: number;
  owned: boolean;
  my_rating: number;
  created_at: string;
  /** Sent only on the user's own packs; `removed` shows 已下架. */
  moderation?: CommunityModeration | null;
};

export type CommunityPluginPage = {
  plugins: CommunityPlugin[];
  has_more: boolean;
  /** Listed packs of a kind this client cannot install (effect packs), left out by the host but still counted toward the next offset. */
  skipped?: number;
};

/** What the host's packer made of an installed pack before anything is uploaded. */
export type CommunityPluginPackPreview = {
  suggestedName: string;
  suggestedDescription: string;
  version: string;
  license: string;
  fileCount: number;
  size: number;
};

/** Desktop community commands for plugin packs. The host packs an installed pack itself and installs a download through the same import a local `.zip` goes through, so the webview names a pack by kind and id, or a publication by its id, and never passes paths or bytes. */
export interface CommunityPluginClient {
  list(
    offset: number,
    search: string,
    kind: CommunityPluginKind | null,
    /** Only the signed-in user's own packs, removed ones included. */
    mine?: boolean,
  ): Promise<CommunityPluginPage>;
  detail(id: string): Promise<CommunityPlugin>;
  packPreview(kind: CommunityPluginKind, pluginId: string): Promise<CommunityPluginPackPreview>;
  /** `id` is the page's publication id: reusing it after a failed attempt lets the server answer a retry with the item it already stored. */
  publish(
    kind: CommunityPluginKind,
    pluginId: string,
    id: string,
    name: string,
    description: string,
  ): Promise<CommunityPlugin>;
  /** Downloads, verifies and installs, replacing an installed pack of the same kind and id. `kind` and `pluginId` are the listing's, the pack the page asked about replacing; the host refuses a download that names another. */
  install(id: string, kind: CommunityPluginKind, pluginId: string): Promise<PluginPackage>;
  rate(id: string, stars: number): Promise<{ stars: number }>;
  delete(id: string): Promise<{ deleted: boolean }>;
  /** Reports another user's pack to the moderators. */
  report?(id: string, reason: CommunityReportReason, detail: string): Promise<void>;
}

/** The server's archive limit, stated next to the packed size so the author sees the headroom. */
const archiveLimit = "8 MB";

const publishWarning =
  "上传的是插件目录中的全部文件（不含隐藏文件），压缩后不超过 8 MB；每个账号最多发布 20 个、合计 32 MB，每小时最多发布 10 次。其他用户下载后会按原样安装。";

function isCommunityKind(kind: PluginKind): kind is CommunityPluginKind {
  return kind !== "effect";
}

function CommunityPluginCard({ plugin, open }: { plugin: CommunityPlugin; open: () => void }) {
  return (
    <button
      type="button"
      className={style.card}
      aria-label={`查看插件 ${plugin.name}`}
      onClick={open}
    >
      <strong className={style.cardTitle}>{plugin.name}</strong>
      <CommunityCardAuthor
        prefix={kindLabels[plugin.kind]}
        author={plugin.author}
        owned={plugin.owned}
        removed={plugin.moderation === "removed"}
      />
      {plugin.description && (
        <span className={style.resourceDescription}>{plugin.description}</span>
      )}
      <CommunityCardMetrics
        downloads={plugin.downloads}
        ratingCount={plugin.rating_count}
        ratingAverage={plugin.rating_average}
      />
    </button>
  );
}

/** The community gallery of plugin packs: browse by kind, install into the plugins directory, rate, publish and take down. */
export function CommunityPluginsPage({
  client,
  localPlugins,
  onInstalled,
  onLogin,
}: {
  client: CommunityPluginClient;
  /** The installed packs: the publish choices, and how an install learns it would replace one. */
  localPlugins?: () => Promise<PluginCatalogResult>;
  /** Called once a pack lands in the plugins directory, so a listing of that directory can read it again. */
  onInstalled?: (installed: PluginPackage) => void;
  onLogin?: () => void;
}) {
  // The gallery hook pages by offset and search; the kind filter rides along through a ref so a page appended by 加载更多 keeps the filter the first page was read with.
  const kindFilter = useRef<CommunityPluginKind | null>(null);
  const galleryClient = useMemo<CommunityGalleryClient<CommunityPlugin>>(
    () => ({
      list: async (offset, search, mine) => {
        const page = await client.list(offset, search, kindFilter.current, mine ?? false);
        return { items: page.plugins, has_more: page.has_more, skipped: page.skipped };
      },
      detail: client.detail,
      rate: async (id, stars) => {
        await client.rate(id, stars);
      },
      unpublish: async (id) => {
        await client.delete(id);
      },
      ...(client.report && {
        report: (id: string, reason: CommunityReportReason, detail: string) =>
          client.report!(id, reason, detail),
      }),
    }),
    [client],
  );
  const gallery = useCommunityGallery({
    client: galleryClient,
    errorMessage: communityPluginMessage,
    needsSignIn: communityNeedsSignIn,
  });
  const {
    items: plugins,
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
  const [kind, setKind] = useState<CommunityPluginKind | null>(null);
  const [installed, setInstalled] = useState(false);
  const [confirmReplace, setConfirmReplace] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);

  const changeKind = async (next: CommunityPluginKind | null) => {
    const previous = kindFilter.current;
    if (next === previous) return;
    kindFilter.current = next;
    setKind(next);
    // A failed first page leaves the previous kind's items and offset in place, so the filter goes back with them; otherwise 加载更多 would append the new kind's page at the old kind's offset.
    const listed = await requestList(activeSearch, false);
    if (!listed && kindFilter.current === next) {
      kindFilter.current = previous;
      setKind(previous);
    }
  };

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
        // An install replaces a pack of the same kind and id whole, so ask first. The scan is only a courtesy: when it fails the install goes ahead as the import from a local `.zip` would.
        if (!replace && localPlugins) {
          const catalog = await localPlugins().catch(() => null);
          if (!gallery.isCurrent(currentClient)) return;
          if (
            catalog?.packages.some(
              (item) => item.kind === target.kind && item.id === target.plugin_id,
            )
          ) {
            setConfirmReplace(true);
            return;
          }
        }
        const pack = await client.install(target.id, target.kind, target.plugin_id);
        if (!gallery.isCurrent(currentClient)) return;
        setConfirmReplace(false);
        setInstalled(true);
        onInstalled?.(pack);
      },
      { clearNotice: true },
    );
  };

  const publishDone = async () => {
    setPublishOpen(false);
    setActionNotice("已发布到社区。");
    await requestList(activeSearch, false);
  };

  if (selected) {
    return (
      <CommunityDetailFrame
        backDisabled={actionBusy}
        onBack={closeDetail}
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
      >
        <CommunityDetailHeader
          title={selected.name}
          note={[
            kindLabels[selected.kind],
            selected.author,
            selected.version && `v${selected.version}`,
          ]
            .filter(Boolean)
            .join(" · ")}
          owned={selected.owned}
          moderation={selected.moderation}
          description={selected.description}
        />
        <p className={style.metrics}>
          {[
            selected.plugin_id,
            selected.license && `授权 ${selected.license}`,
            candidateSkinMegabytes(selected.size),
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>
        <CommunityDetailStatus
          downloads={selected.downloads}
          ratingCount={selected.rating_count}
          ratingAverage={selected.rating_average}
          myRating={selected.my_rating}
          detailBusy={detailBusy}
          actionNotice={actionNotice}
          loadingText="正在读取插件详情…"
        />
        {installed ? (
          <CommunityActionNotice>已安装到插件目录，可在「我的插件」中选用。</CommunityActionNotice>
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
            ariaLabel="确认替换插件"
            message={
              <>
                已安装同 id 的{kindLabels[selected.kind]}“{selected.plugin_id}
                ”，安装会整体替换它。
              </>
            }
            actionBusy={actionBusy}
            onConfirm={() => void install(true)}
            onCancel={() => setConfirmReplace(false)}
          />
        )}
        <CommunityModerationSection
          owned={selected.owned}
          actionBusy={actionBusy}
          ratingDescription="我的评分（安装后可评，可重新选择）"
          unpublishMessage={`下架后其他用户无法再下载，下载数和评分会清空；已安装的插件会保留。确定下架“${selected.name}”吗？`}
          confirmUnpublish={confirmUnpublish}
          onRate={(stars) => void rateSelected(stars)}
          onRequestUnpublish={() => setConfirmUnpublish(true)}
          onUnpublish={() =>
            void unpublishSelected("已下架这个插件；其他用户将无法再下载。已安装的插件会保留。")
          }
          onCancelUnpublish={() => setConfirmUnpublish(false)}
          confirmationActionsClassName={style.confirmationActions}
          unpublishLabel="下架这个插件"
          unpublishConfirmLabel="确认下架插件"
        />
        {!selected.owned && client.report && (
          <CommunityReportSection actionBusy={actionBusy} onReport={reportSelected} />
        )}
      </CommunityDetailFrame>
    );
  }

  return (
    <div className={style.page}>
      <CommunitySearchForm
        label="搜索插件"
        value={search}
        onChange={setSearch}
        onSubmit={() => void requestList(search, false)}
      />
      <CommunityGalleryHeading
        title="社区插件"
        note="音效、音乐、指令表、短语表、辅助码表、单词本与符号集，安装后在「我的插件」中选用"
      >
        <CommunityScopeButtons
          ariaLabel="插件范围"
          mineOnly={mineOnly}
          allLabel="全部插件"
          mineLabel="我的作品"
          onMineOnlyChange={(nextMineOnly) => {
            setMineOnly(nextMineOnly);
            void requestList(activeSearch, false, nextMineOnly);
          }}
        />
        {localPlugins && (
          <ActionButton
            action={() => setPublishOpen(true)}
            className="primary"
            label="发布我的插件"
          />
        )}
      </CommunityGalleryHeading>
      <div className={style.kindFilter} role="group" aria-label="插件类型">
        <ActionButton
          action={() => changeKind(null)}
          className={kind === null ? "primary" : "secondary"}
          ariaPressed={kind === null}
          label="全部"
        />
        {communityPluginKinds.map((item) => (
          <ActionButton
            key={item}
            action={() => changeKind(item)}
            className={kind === item ? "primary" : "secondary"}
            ariaPressed={kind === item}
            label={kindLabels[item]}
          />
        ))}
      </div>
      <CommunityGalleryFeedback
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
        notice={actionNotice && <CommunityActionNotice>{actionNotice}</CommunityActionNotice>}
        empty={
          !listBusy && !error && plugins.length === 0 ? (
            hasMore ? (
              <p className={style.notice}>
                {activeSearch || kind
                  ? "前面的插件这台设备都不能安装，点「加载更多」继续查找。"
                  : "前面的插件这台设备都不能安装，点「加载更多」查看更早发布的插件。"}
              </p>
            ) : (
              <p className={style.notice}>
                {mineOnly
                  ? "你还没有发布过插件。"
                  : activeSearch || kind
                    ? "没有匹配的插件。"
                    : localPlugins
                      ? "社区里还没有插件，安装或制作插件后可以点「发布我的插件」分享出来。"
                      : "社区里还没有插件。"}
              </p>
            )
          ) : undefined
        }
      />
      <div className={style.grid}>
        {plugins.map((plugin) => (
          <CommunityPluginCard key={plugin.id} plugin={plugin} open={() => open(plugin)} />
        ))}
      </div>
      <CommunityGalleryLoadMore
        hasMore={hasMore}
        busy={listBusy}
        loadingText="正在读取插件…"
        onLoadMore={() => void requestList(activeSearch, true)}
      />
      {publishOpen && localPlugins && (
        <CommunityPluginPublishDialog
          client={client}
          localPlugins={localPlugins}
          onClose={() => setPublishOpen(false)}
          onPublished={publishDone}
          onLogin={communityPublishLoginAction(() => setPublishOpen(false), onLogin)}
        />
      )}
    </div>
  );
}

/** `label` 是类型显示名：本机的包知道自己是不是旋律音效包，所以用 `packKindLabel`；社区列表的条目不带 `mode`，只能用 `kindLabels`。 */
type LocalPluginOption = {
  key: string;
  kind: CommunityPluginKind;
  label: string;
  id: string;
  name: string;
};

function optionKey(kind: PluginKind, id: string): string {
  return `${kind}/${id}`;
}

/**
 * Publishes one of the user's installed, non-built-in plugin packs.
 *
 * The host packs the pack from its own plugins directory, so the webview only names it by kind and id; `packPreview` runs the same packer the upload will, which lets a pack the server would refuse say so before the form is filled in. The publication id is drawn afresh whenever the pack, the name or the description changes and kept across a failed attempt, so a retry after a lost response is recognised by the server as the same publication.
 */
export function CommunityPluginPublishDialog({
  client,
  localPlugins,
  onClose,
  onPublished,
  onLogin,
}: {
  client: CommunityPluginClient;
  localPlugins: () => Promise<PluginCatalogResult>;
  onClose: () => void;
  onPublished: (plugin: CommunityPlugin) => void | Promise<void>;
  onLogin?: () => void;
}) {
  const [options, setOptions] = useState<LocalPluginOption[]>([]);
  const [optionsLoading, setOptionsLoading] = useState(true);
  const [selection, setSelection] = useState("");
  const [pack, setPack] = useState<CommunityPluginPackPreview | null>(null);
  const [packError, setPackError] = useState("");
  const [packLoading, setPackLoading] = useState(false);
  const {
    name,
    description,
    agreed,
    publicationId,
    setName,
    setDescription,
    setAgreed,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    resetPublication,
  } = useCommunityPublicationDraft();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  const clientGeneration = useRef(0);
  const packGeneration = useRef(0);
  const actionRunning = useRef(false);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    let active = true;
    actionRunning.current = false;
    setBusy(false);
    setOptionsLoading(true);
    void localPlugins()
      .then((catalog) => {
        if (!active) return;
        const loaded: LocalPluginOption[] = [];
        for (const item of catalog.packages) {
          if (item.builtin || !isCommunityKind(item.kind)) continue;
          loaded.push({
            key: optionKey(item.kind, item.id),
            kind: item.kind,
            label: packKindLabel(item),
            id: item.id,
            name: item.name,
          });
        }
        setOptions(loaded);
        setSelection((current) =>
          current && loaded.some((item) => item.key === current) ? current : (loaded[0]?.key ?? ""),
        );
      })
      .catch(() => {
        if (active) setError("读取本地插件失败，请重试。");
      })
      .finally(() => {
        if (active) setOptionsLoading(false);
      });
    return () => {
      active = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client, localPlugins]);

  const chosen = useMemo(
    () => options.find((item) => item.key === selection) ?? null,
    [options, selection],
  );

  useEffect(() => {
    const generation = ++packGeneration.current;
    setPack(null);
    setPackError("");
    setAgreed(false);
    resetPublication();
    if (!chosen) {
      setPackLoading(false);
      return;
    }
    setPackLoading(true);
    void client
      .packPreview(chosen.kind, chosen.id)
      .then((value) => {
        if (generation !== packGeneration.current) return;
        setPack(value);
        setName(boundedGraphemes(value.suggestedName, 32));
        setDescription(value.suggestedDescription);
      })
      .catch((packFailure) => {
        if (generation === packGeneration.current)
          setPackError(communityPluginMessage(packFailure));
      })
      .finally(() => {
        if (generation === packGeneration.current) setPackLoading(false);
      });
    return () => {
      packGeneration.current++;
    };
  }, [client, chosen]);

  const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
    communityPublishFields(name, description);
  const ready = Boolean(pack) && !packLoading && nameValid && descriptionValid && agreed;

  const submit = async () => {
    if (busy || actionRunning.current || !ready || !chosen) return;
    const generation = clientGeneration.current;
    await runCommunityPublishAction({
      busy,
      generation,
      clientGeneration,
      actionRunning,
      setBusy,
      setError,
      setSignInRequired,
      formatError: (publishError) => communityPluginMessage(publishError, true),
      operation: async (isCurrent) => {
        const published = await client.publish(
          chosen.kind,
          chosen.id,
          publicationId,
          normalizedName,
          normalizedDescription,
        );
        if (!isCurrent()) return;
        await onPublished(published);
      },
    });
  };

  return (
    <CommunityDialogFrame
      title="发布插件"
      titleClassName={style.dialogTitle}
      ariaLabel="发布插件"
      busy={busy}
      onClose={onClose}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
      onKeyDown={(event) => handleCommunityPublishKeyDown(event, () => void submit())}
    >
      {optionsLoading && <p role="status">正在读取本地插件…</p>}
      {!optionsLoading && options.length === 0 && (
        <p className={style.notice}>
          还没有可发布的插件。内置插件和特效包不能发布，请先在「我的插件」中导入自己的音效包、音乐包、指令表、短语表、辅助码表、单词本或符号集。
        </p>
      )}
      {options.length > 0 && (
        <CommunitySelectField
          label="发布插件"
          ariaLabel="发布插件"
          value={selection}
          disabled={busy}
          onChange={setSelection}
        >
          {options.map((item) => (
            <option key={item.key} value={item.key}>
              {item.label} · {item.name === item.id ? item.id : `${item.name}（${item.id}）`}
            </option>
          ))}
        </CommunitySelectField>
      )}
      {packLoading && <p role="status">正在检查插件…</p>}
      {packError && (
        <div className={style.confirmation} role="alert">
          <p>{packError}</p>
        </div>
      )}
      {pack && (
        <>
          <p className={style.metrics}>
            {[
              `v${pack.version}`,
              pack.license && `授权 ${pack.license}`,
              `${pack.fileCount} 个文件`,
              `${candidateSkinMegabytes(pack.size)} / ${archiveLimit}`,
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
          <CommunityPublicationMetadataFields
            name={name}
            description={description}
            agreed={agreed}
            busy={busy}
            nameLabel="名称"
            nameAriaLabel="发布插件名称"
            descriptionLabel="说明"
            descriptionAriaLabel="发布插件说明"
            agreementText="我拥有插件中音频与文字的发布权利，并同意其他用户按包内授权免费下载使用"
            agreementClassName={style.agreementBox}
            onNameChange={onNameChange}
            onDescriptionChange={onDescriptionChange}
            onAgreedChange={onAgreedChange}
          />
          <CommunityPublicationWarning>{publishWarning}</CommunityPublicationWarning>
        </>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        {!packError && (
          <ActionButton
            action={() => submit()}
            className="primary"
            disabled={busy || !ready}
            label={busy ? "正在发布…" : "公开发布"}
          />
        )}
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}
