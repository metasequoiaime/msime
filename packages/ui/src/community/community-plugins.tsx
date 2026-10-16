import { useMemo, useRef, useState } from "react";
import type { PluginCatalogResult, PluginPackage } from "../settings/plugin-types";
import { kindLabels } from "../settings/plugin-catalog-helpers";
import {
  candidateSkinMegabytes,
  communityNeedsSignIn,
  communityPluginMessage,
  communityPublishLoginAction,
} from "./community-helpers";
import {
  createCommunityGalleryClient,
  useCommunityGallery,
  type CommunityGalleryClient,
} from "./community-gallery";
import { CommunityDetailSummary } from "./community-detail-summary";
import * as style from "./community-style";
import { CommunityGalleryHeader } from "./community-gallery-header";
import { CommunityModerationSection } from "./community-moderation-section";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunityNotice } from "./community-notice";
import { CommunityReportSection } from "./community-report";
import { CommunityCardMetrics } from "./community-card-metrics";
import { CommunityCardAuthor } from "./community-card-author";
import { CommunityCard } from "./community-card";
import { CommunityMetrics } from "./community-metrics";
import { CommunityInstallActions } from "./community-install-actions";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { CommunityDetailFrame } from "./community-detail-frame";
import { ActionButton } from "../core/action-button";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { useCommunityInstallState } from "./community-install-state";
import { communityInstallPreflight } from "./community-install-preflight";
import { CommunityPluginPublishDialog } from "./community-plugin-publish-dialog";

import {
  type CommunityPluginKind,
  communityPluginKinds,
  type CommunityPlugin,
  type CommunityPluginClient,
} from "./community-plugin-model";
export {
  type CommunityPluginKind,
  communityPluginKinds,
  type CommunityPlugin,
  type CommunityPluginPage,
  type CommunityPluginPackPreview,
  type CommunityPluginClient,
} from "./community-plugin-model";

export {
  CommunityPluginPublishDialog,
  type CommunityPluginPublishDialogProps,
} from "./community-plugin-publish-dialog";

function CommunityPluginCard({ plugin, open }: { plugin: CommunityPlugin; open: () => void }) {
  return (
    <CommunityCard aria-label={`查看插件 ${plugin.name}`} onClick={open}>
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
    </CommunityCard>
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
    () =>
      createCommunityGalleryClient(
        client,
        async (offset, search, mine) => {
          const page = await client.list(offset, search, kindFilter.current, mine ?? false);
          return { items: page.plugins, has_more: page.has_more, skipped: page.skipped };
        },
        (id) => client.delete(id),
      ),
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
    changeMineOnly,
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
  const [publishOpen, setPublishOpen] = useState(false);
  const { installed, setInstalled, confirmReplace, setConfirmReplace, closeDetail } =
    useCommunityInstallState(actionBusy, closeGalleryDetail);

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

  const install = async (replace: boolean) => {
    if (!selected) return;
    const target = selected;
    setInstalled(false);
    await runAction(
      async (currentClient) => {
        // An install replaces a pack of the same kind and id whole, so ask first. The scan is only a courtesy: when it fails the install goes ahead as the import from a local `.zip` would.
        if (!replace && localPlugins) {
          const preflight = await communityInstallPreflight(
            localPlugins,
            () => gallery.isCurrent(currentClient),
            (catalog) =>
              catalog.packages.some(
                (item) => item.kind === target.kind && item.id === target.plugin_id,
              ),
          );
          if (preflight === "stale") return;
          if (preflight === "conflict") {
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
        <CommunityDetailSummary
          title={selected.name}
          noteParts={[
            kindLabels[selected.kind],
            selected.author,
            selected.version && `v${selected.version}`,
          ]}
          owned={selected.owned}
          moderation={selected.moderation}
          description={selected.description}
          metrics={
            <CommunityMetrics>
              {[
                selected.plugin_id,
                selected.license && `授权 ${selected.license}`,
                candidateSkinMegabytes(selected.size),
              ]
                .filter(Boolean)
                .join(" · ")}
            </CommunityMetrics>
          }
          downloads={selected.downloads}
          ratingCount={selected.rating_count}
          ratingAverage={selected.rating_average}
          myRating={selected.my_rating}
          detailBusy={detailBusy}
          actionNotice={actionNotice}
          loadingText="正在读取插件详情…"
        />
        <CommunityInstallActions
          installed={installed}
          installedContent={
            <CommunityActionNotice>
              已安装到插件目录，可在「我的插件」中选用。
            </CommunityActionNotice>
          }
          actionBusy={actionBusy}
          detailBusy={detailBusy}
          confirmReplace={confirmReplace}
          replaceAriaLabel="确认替换插件"
          replaceMessage={
            <>
              已安装同 id 的{kindLabels[selected.kind]}“{selected.plugin_id}
              ”，安装会整体替换它。
            </>
          }
          onInstall={(replace) => void install(replace)}
          onCancelReplace={() => setConfirmReplace(false)}
        />
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
    <CommunityPageShell>
      <CommunityGalleryHeader
        searchLabel="搜索插件"
        search={search}
        onSearchChange={setSearch}
        onSearchSubmit={() => void requestList(search, false)}
        title="社区插件"
        note="音效、音乐、指令表、特效、短语表、辅助码表、单词本与符号集，安装后在「我的插件」中选用"
      >
        <CommunityScopeButtons
          ariaLabel="插件范围"
          mineOnly={mineOnly}
          allLabel="全部插件"
          mineLabel="我的作品"
          onMineOnlyChange={changeMineOnly}
        />
        {localPlugins && (
          <ActionButton
            action={() => setPublishOpen(true)}
            className="primary"
            label="发布我的插件"
          />
        )}
      </CommunityGalleryHeader>
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
              <CommunityNotice>
                {activeSearch || kind
                  ? "前面的插件这台设备都不能安装，点「加载更多」继续查找。"
                  : "前面的插件这台设备都不能安装，点「加载更多」查看更早发布的插件。"}
              </CommunityNotice>
            ) : (
              <CommunityNotice>
                {mineOnly
                  ? "你还没有发布过插件。"
                  : activeSearch || kind
                    ? "没有匹配的插件。"
                    : localPlugins
                      ? "社区里还没有插件，安装或制作插件后可以点「发布我的插件」分享出来。"
                      : "社区里还没有插件。"}
              </CommunityNotice>
            )
          ) : undefined
        }
      />
      <CommunityGalleryGrid>
        {plugins.map((plugin) => (
          <CommunityPluginCard key={plugin.id} plugin={plugin} open={() => open(plugin)} />
        ))}
      </CommunityGalleryGrid>
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
    </CommunityPageShell>
  );
}
