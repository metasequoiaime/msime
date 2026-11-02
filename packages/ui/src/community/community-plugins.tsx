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

import {
  communityPluginKindsFor,
  type CommunityPluginClient,
  type CommunityPluginKind,
} from "./community-plugin-model";
export {
  type CommunityPluginKind,
  communityPluginKinds,
  communityPluginKindsFor,
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
  onOpenScript,
  scriptPlugins = false,
  kinds: shownKinds,
  mobile = false,
  onLogin,
  contributions = true,
}: {
  client: CommunityPluginClient;
  /** 已安装包目录，供发布选择与安装替换预检使用。 */
  localPlugins?: () => Promise<PluginCatalogResult>;
  /** 安装完成后通知调用方重新读取本机插件目录。 */
  onInstalled?: (installed: PluginPackage) => void;
  /** 装好脚本插件后打开它的授权页。 */
  onOpenScript?: (installed: PluginPackage) => void;
  /** 宿主运行脚本插件（`HostCapabilities.script_plugins`）：可以按「脚本插件」筛选、发布自己的脚本插件。 */
  scriptPlugins?: boolean;
  /** 只列、只筛这几类（触屏插件页，见 `touchPluginKinds`）；缺省时按 `scriptPlugins` 给全部可分享的类型。 */
  kinds?: readonly CommunityPluginKind[];
  /** 触屏设置：详情推入历史记录，系统返回键关掉它。 */
  mobile?: boolean;
  onLogin?: () => void;
  /** 宿主提供社区投稿类功能（`HostCapabilities.community_contribution`）。为 false 时（HarmonyOS 在实名认证到位之前）没有「发布我的插件」，详情里也没有评分和举报；浏览、安装和下架自己的插件照常。缺省为提供。 */
  contributions?: boolean;
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
    blocked,
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
  } = useCommunityPluginGallery({
    client,
    localPlugins,
    onInstalled,
    onOpenScript,
    kinds: shownKinds,
    mobile,
  });
  const kinds = shownKinds ?? communityPluginKindsFor(scriptPlugins);
  // 能发布的前提：宿主提供投稿类功能，并且读得到本机已安装的插件。
  const publishable = contributions && Boolean(localPlugins);

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
        onRate={contributions ? (stars) => void rateSelected(stars) : undefined}
        onRequestUnpublish={() => setConfirmUnpublish(true)}
        onUnpublish={() =>
          void unpublishSelected("已下架这个插件；其他用户将无法再下载。已安装的插件会保留。")
        }
        onCancelUnpublish={() => setConfirmUnpublish(false)}
        onReport={contributions && client.report ? reportSelected : undefined}
        onBlocked={(notice) => void blocked(notice)}
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
        help={
          shownKinds
            ? `${shownKinds.map((item) => kindLabels[item]).join("、")}，安装后在「我的插件」中选用；这台设备用不上的类型不在这里列出`
            : scriptPlugins
              ? "音效、音乐、指令表、特效、短语表、辅助码表、单词本、符号集与脚本插件，安装后在「我的插件」中选用；脚本插件经管理员审核后才会出现，安装后要授权才会运行"
              : "音效、音乐、指令表、特效、短语表、辅助码表、单词本与符号集，安装后在「我的插件」中选用"
        }
      >
        <CommunityScopeButtons
          ariaLabel="插件范围"
          mineOnly={mineOnly}
          allLabel="全部插件"
          mineLabel="我的作品"
          onMineOnlyChange={changeMineOnly}
        />
        {publishable && (
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
        {kinds.map((item) => (
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
                    : publishable
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
      {publishOpen && publishable && localPlugins && (
        <CommunityPluginPublishDialog
          client={client}
          localPlugins={localPlugins}
          scriptPlugins={scriptPlugins}
          onClose={() => setPublishOpen(false)}
          onPublished={publishDone}
          onLogin={communityPublishLoginAction(() => setPublishOpen(false), onLogin)}
        />
      )}
    </CommunityPageShell>
  );
}
