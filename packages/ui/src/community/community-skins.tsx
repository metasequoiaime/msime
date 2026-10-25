// Source: MSIME-Apple@9ca823ab40018ced3cb71812503dbc3b94615ac0
// (`SkinCommunityView.swift`, `CommunityGalleryStyle.swift`).
import { communityPublishLoginAction } from "./community-helpers";
import { CommunitySkinDetail } from "./community-skin-detail";
import * as style from "./community-style";
import { CommunitySearchForm } from "./community-search-form";
export { CommunityHarmonySearch } from "./community-search-form";
export { CommunityHarmonyLoadMore } from "./community-gallery-load-more";
import { CommunityScopeButtons } from "./community-scope-buttons";
import { CommunityNotice } from "./community-notice";
import { CommunitySkinPublishDialog } from "./community-skin-publish-dialog";
import { CommunitySkinCard } from "./community-skin-card";
import { CommunityHarmonySkinCard } from "./community-harmony-skin-card";
import { CommunityGalleryLoadMore } from "./community-gallery-load-more";
import { CommunityGalleryFeedback } from "./community-gallery-feedback";
import { CommunityGalleryHeader } from "./community-gallery-header";
import { CommunityGalleryGrid } from "./community-gallery-grid";
import { CommunityPageShell } from "./community-page-shell";
import { ActionButton } from "../core/action-button";
import { useToast } from "../core/toast";
import type { Preferences } from "../core/preferences-model";
import { CommunitySkinCategoryFilter } from "./community-skin-category";
import {
  communitySkinCategories,
  communitySkinCategoryLabels,
} from "./community-skin-category-model";
import { useCommunitySkinLibrary } from "./use-community-skin-library";
import { useCommunitySkinGallery } from "./use-community-skin-gallery";
import type { CustomSkinLibraryClient } from "../keyboard/touch-keyboard-skin-design";
import { harmonyTouchRowsForPreferences } from "../keyboard/keyboard-layouts";

import { type CommunitySkinClient } from "./community-skin-model";
export {
  type CommunitySkin,
  type CommunitySkinPage,
  type CommunitySkinTrial,
  type CommunitySkinDownload,
  type CommunitySkinClient,
} from "./community-skin-model";

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
  historyPage = "community",
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
  /** 手机上打开详情时压入的历史记录所属的设置页。触屏宿主在「皮肤」页里打开这个图库，记录要留在那一页，返回手势才会先关详情、再关图库；缺省是桌面的「社区」页。 */
  historyPage?: string;
}) {
  const {
    categoryFilter,
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
    search,
    setSearch,
    trial,
    publishOpen,
    setPublishOpen,
    changeMineOnly,
    setConfirmUnpublish,
    requestList,
    open,
    closeDetail,
    rateSelected,
    unpublish,
    reportSelected,
    download,
    finishTrial,
    changeCategory,
    changeOwnCategory,
    publishDone,
  } = useCommunitySkinGallery({
    client,
    initialMine,
    mobile,
    historyPage,
    harmony: look === "harmony",
  });

  const harmony = look === "harmony";
  const toast = useToast();
  const { pillState, take, apply } = useCommunitySkinLibrary({
    client,
    localSkinLibrary,
    harmony,
    browsing: selected === null,
    preferences,
    onApplyPreferences,
    onSkinApplied,
    toast,
  });

  if (selected)
    return (
      <CommunitySkinDetail
        skin={selected}
        theme={theme}
        phonePreview={mobile}
        touchRows={harmony && preferences ? harmonyTouchRowsForPreferences(preferences) : undefined}
        actionBusy={actionBusy}
        detailBusy={detailBusy}
        actionNotice={actionNotice}
        confirmUnpublish={confirmUnpublish}
        trial={trial}
        error={error}
        signInRequired={signInRequired}
        onLogin={onLogin}
        onBack={() => void closeDetail()}
        onDownload={() => void download()}
        onFinishTrial={(keep) => void finishTrial(keep)}
        onChangeCategory={(next) => void changeOwnCategory(next)}
        onRate={(stars) => void rateSelected(stars)}
        onRequestUnpublish={() => setConfirmUnpublish(true)}
        onUnpublish={() => void unpublish()}
        onCancelUnpublish={() => setConfirmUnpublish(false)}
        onReport={client.report ? reportSelected : undefined}
      />
    );

  const publishDialog = publishOpen && localSkinLibrary && (
    <CommunitySkinPublishDialog
      client={client}
      library={localSkinLibrary}
      phonePreview={mobile}
      touchRows={harmony && preferences ? harmonyTouchRowsForPreferences(preferences) : undefined}
      onClose={() => setPublishOpen(false)}
      onPublished={publishDone}
      onLogin={communityPublishLoginAction(() => setPublishOpen(false), onLogin)}
    />
  );

  if (harmony) {
    const shown = skins.filter((skin) => !mineOnly || skin.owned);
    return (
      <div className={style.harmonyPage}>
        <CommunitySearchForm
          look="harmony"
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
              changeMineOnly(!mineOnly);
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
                  touchRows={preferences ? harmonyTouchRowsForPreferences(preferences) : undefined}
                  pill={pill}
                  open={() => open(skin)}
                  act={() => void (pill === "use" ? apply(skin) : take(skin))}
                />
              );
            })}
          </div>
        )}
        <CommunityGalleryLoadMore
          look="harmony"
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
      <CommunityGalleryHeader
        searchLabel="搜索皮肤设计"
        search={search}
        onSearchChange={setSearch}
        onSearchSubmit={() => void requestList(search, false)}
        title={mineOnly ? "你的公开设计" : "换个心情，从键盘开始"}
        note={mineOnly ? "管理你发布到社区的皮肤" : "发现创作者的配色与巧思，找到你的那一款"}
      >
        <CommunityScopeButtons
          ariaLabel="社区皮肤范围"
          mineOnly={mineOnly}
          allLabel="全部皮肤"
          mineLabel="我的作品"
          onMineOnlyChange={changeMineOnly}
        />
        {localSkinLibrary && (
          <ActionButton
            action={() => setPublishOpen(true)}
            className="primary"
            label="发布我的设计"
          />
        )}
      </CommunityGalleryHeader>
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
            <CommunitySkinCard
              key={skin.id}
              skin={skin}
              theme={theme}
              phonePreview={mobile}
              touchRows={
                harmony && preferences ? harmonyTouchRowsForPreferences(preferences) : undefined
              }
              open={() => open(skin)}
            />
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
