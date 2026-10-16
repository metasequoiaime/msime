import type { PluginCatalogResult, PluginPackage } from "../settings/plugin-types";
import { kindLabels } from "../settings/plugin-catalog-helpers";
import { communityPublishLoginAction } from "./community-helpers";
import { useCommunityPluginGallery } from "./use-community-plugin-gallery";
import * as style from "./community-style";
import { CommunityGalleryHeader } from "./community-gallery-header";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunityNotice } from "./community-notice";
import { CommunityPluginCard } from "./community-plugin-card";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { ActionButton } from "../core/action-button";
import { CommunityActionNotice } from "./community-action-notice";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { CommunityPluginPublishDialog } from "./community-plugin-publish-dialog";
import { CommunityPluginDetail } from "./community-plugin-detail";

import { communityPluginKinds, type CommunityPluginClient } from "./community-plugin-model";
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

/** 社区插件包图库：按类型浏览、安装、评分、发布与下架。 */
export function CommunityPluginsPage({
  client,
  localPlugins,
  onInstalled,
  onLogin,
}: {
  client: CommunityPluginClient;
  /** 已安装包目录，供发布选择与安装替换预检使用。 */
  localPlugins?: () => Promise<PluginCatalogResult>;
  /** 安装完成后通知调用方重新读取本机插件目录。 */
  onInstalled?: (installed: PluginPackage) => void;
  onLogin?: () => void;
}) {
  const {
    plugins,
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
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    unpublishSelected,
    reportSelected,
    search,
    setSearch,
    kind,
    changeKind,
    publishOpen,
    setPublishOpen,
    installed,
    confirmReplace,
    setConfirmReplace,
    install,
    publishDone,
  } = useCommunityPluginGallery({ client, localPlugins, onInstalled });

  if (selected) {
    return (
      <CommunityPluginDetail
        plugin={selected}
        actionBusy={actionBusy}
        detailBusy={detailBusy}
        actionNotice={actionNotice}
        error={error}
        signInRequired={signInRequired}
        installed={installed}
        confirmReplace={confirmReplace}
        confirmUnpublish={confirmUnpublish}
        onLogin={onLogin}
        onBack={closeDetail}
        onInstall={(replace) => void install(replace)}
        onCancelReplace={() => setConfirmReplace(false)}
        onRate={(stars) => void rateSelected(stars)}
        onRequestUnpublish={() => setConfirmUnpublish(true)}
        onUnpublish={() =>
          void unpublishSelected("已下架这个插件；其他用户将无法再下载。已安装的插件会保留。")
        }
        onCancelUnpublish={() => setConfirmUnpublish(false)}
        onReport={client.report ? reportSelected : undefined}
      />
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
