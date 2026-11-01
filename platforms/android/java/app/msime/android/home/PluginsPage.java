package app.msime.android.home;

import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.view.inputmethod.EditorInfo;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.DimensionPolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginEditorPolicy;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/**
 * 插件页：「我的插件」和「社区插件」两个分类，与共享设置页的插件页同样的内容和文字，只列手机上生效的类型。
 *
 * <p>「我的插件」按类型分组列出已安装的包：音效包（只有按键音效包，按键旋律手机上不播）、指令表、短语表、辅助码表、单词本和符号集，规则在 {@link PluginCatalogPolicy}；特效包、音乐包、按键旋律和脚本插件不列，只在末尾说有几个。偏好里指着已不在的包的选择也列在对应分组里，点开可以去掉。下面是「新建插件」（{@link PluginEditorPage}）和「导入 .zip」：系统文件选择器（SAF）选中的压缩包先复制进私有暂存目录，再经 `msime_client_plugins` 的 `import` 安装（{@link PluginStore#importArchive}）；手机上不提供导入文件夹。
 *
 * <p>「社区插件」经 `msime_client_community` 的 plugin_* 操作浏览、搜索、按类型筛选，点开是 {@link CommunityPluginPage}（安装、评分、举报、下架），「发布我的插件」是 {@link PluginPublishPage}。列表只列手机上生效的类型；不按类型筛选时服务端也会给出特效包、音乐包，这里去掉并按「这台设备不能安装」的那一页处理，「加载更多」照常往后翻。
 *
 * <p>插件目录是键盘读的那一份（{@link HostStore#directory} 下的 `plugins/`），这里装上、删掉或启用的包，键盘在下一次获得焦点时用上。
 */
public final class PluginsPage extends ReloadingDetailPage {
    static final String MINE = "我的插件";
    static final String COMMUNITY = "社区插件";
    /** SAF 给 .zip 报的 MIME 因文件管理器而异，一并放行；内容由 client-services 校验。 */
    private static final String[] ARCHIVE_TYPES = {"application/zip", "application/x-zip-compressed", "application/octet-stream"};

    @Override protected List<String> sectionTitles() {
        return List.of(MINE, COMMUNITY);
    }

    /** 一次读到的目录和偏好；目录读不出来时 `catalog` 为 null。 */
    private record State(@Nullable PluginStore.Catalog catalog, @Nullable PluginCatalogPolicy.Selection selection) {}

    @Nullable private State state;
    private boolean loaded;
    /** 导入或删除之后的一句提示，留到下一次离开这一页。 */
    private String notice = "";
    private boolean importing;

    // ---- 社区插件的列表状态：跨重绘保留，切回这一页不重读 ----
    private final List<PluginStore.CommunityPlugin> community = new ArrayList<>();
    private boolean communityLoaded;
    private boolean communityLoading;
    private boolean communityHasMore;
    /** 下一页的偏移：已列出的条数加上被去掉的不能安装的条数。 */
    private int communityOffset;
    private String communityError = "";
    private boolean communitySignIn;
    private String communitySearch = "";
    private String communityQuery = "";
    @Nullable private String communityKind;
    private boolean communityMine;
    private long communityGeneration;
    /** 上一次读社区列表时的 {@link #communityRevision}；社区插件页装好、下架或发布过之后两者不等，回到这一页时从头读一次。 */
    private long communitySeen;
    /** 社区里本机账号的作品或本机的包变过一次就加一（{@link #communityChanged}）。 */
    private static volatile long communityRevision;

    private final ActivityResultLauncher<String[]> openArchive =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onArchivePicked);

    @Override public void onSaveInstanceState(@NonNull Bundle out) {
        super.onSaveInstanceState(out);
        out.putString("plugins-community-search", communitySearch);
    }

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        if (saved != null) communitySearch = saved.getString("plugins-community-search", "");
    }

    @Override protected void reload() {
        HostTask.run(this, PluginsPage::read, result -> {
            state = result;
            loaded = true;
            render();
        });
        if (!communityLoading && (!communityLoaded || communitySeen != communityRevision)) loadCommunity(false);
    }

    @Nullable private static State read(Context context) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        return new State(PluginStore.catalog(context),
            preferences == null ? null : PluginStore.selection(context, preferences));
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        renderMine(sectionColumn(target, MINE));
        renderCommunity(sectionColumn(target, COMMUNITY));
    }

    // ---- 我的插件 ----

    private void renderMine(LinearLayout column) {
        Context context = requireContext();
        PluginStore.Catalog catalog = state == null ? null : state.catalog();
        PluginCatalogPolicy.Selection selection = state == null ? null : state.selection();
        if (!loaded) {
            GroupCard.add(column, null).note("正在读取插件…");
        } else if (catalog == null || selection == null) {
            GroupCard.add(column, null).note("插件列表没有读取成功，重新打开这个页面会再试一次。");
        } else {
            List<PluginCatalogPolicy.Pack> packs = new ArrayList<>(catalog.entries().size());
            for (PluginStore.Entry entry : catalog.entries()) packs.add(entry.pack());
            List<PluginCatalogPolicy.Missing> missing = PluginCatalogPolicy.missingSelections(selection, packs);
            for (String kind : PluginCatalogPolicy.PHONE_KINDS) {
                List<PluginCatalogPolicy.Pack> shown = new ArrayList<>();
                for (PluginCatalogPolicy.Pack pack : packs) {
                    if (pack.kind().equals(kind) && PluginCatalogPolicy.usedOnPhone(pack)) shown.add(pack);
                }
                List<PluginCatalogPolicy.Missing> gone = new ArrayList<>();
                for (PluginCatalogPolicy.Missing entry : missing) if (entry.kind().equals(kind)) gone.add(entry);
                boolean commands = PluginCatalogPolicy.COMMAND_TABLE.equals(kind);
                if (shown.isEmpty() && gone.isEmpty() && !commands) continue;
                GroupCard group = GroupCard.add(column, PluginCatalogPolicy.kindLabel(kind)).withDividers(Ui.ROW_PADDING_H);
                if (commands) {
                    group.help("打开 / 指令后，在键盘的「本地输入」里点「指令」，或在没有输入时按 / 键，再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用 "
                        + PluginCatalogPolicy.MAX_TABLES + " 个。");
                    if (shown.isEmpty() && gone.isEmpty()) group.note("还没有导入指令表。");
                }
                for (PluginCatalogPolicy.Pack pack : shown) {
                    group.nav(pack.name(), PluginCatalogPolicy.rowDescription(pack),
                        PluginCatalogPolicy.marker(pack, selection),
                        () -> PluginDetailPage.open(context, pack.kind(), pack.id(), false, false), true);
                }
                for (PluginCatalogPolicy.Missing entry : gone) {
                    group.nav(PluginCatalogPolicy.missingTitle(entry), PluginCatalogPolicy.missingDescription(entry), null,
                        () -> PluginDetailPage.open(context, entry.kind(), entry.id(), true, false));
                }
            }
            if (packs.stream().noneMatch(PluginCatalogPolicy::usedOnPhone) && missing.isEmpty()) {
                GroupCard.add(column, null).note("没有插件");
            }
            List<PluginStore.Issue> issues = new ArrayList<>();
            for (PluginStore.Issue issue : catalog.issues()) if (!"script".equals(issue.kind())) issues.add(issue);
            if (!issues.isEmpty()) {
                GroupCard broken = GroupCard.add(column, "无法载入的插件");
                for (PluginStore.Issue issue : issues) {
                    broken.note(PluginCatalogPolicy.kindLabel(issue.kind()) + " "
                        + (issue.folder().isEmpty() ? "目录" : issue.folder()) + " 无法载入：" + issue.reason());
                }
            }
            String unused = PluginCatalogPolicy.unusedNote(PluginCatalogPolicy.unusedCount(packs));
            if (unused != null) GroupCard.add(column, null).note(unused);
        }

        GroupCard create = GroupCard.add(column, "制作插件");
        create.button("自己制作", null, "新建插件",
            () -> PluginEditorPage.create(context)).help(PluginEditorPolicy.CREATE_DESCRIPTION);
        GroupCard importing = GroupCard.add(column, "导入插件");
        GroupCard.Row row = importing.button("从文件导入", null,
            "导入 .zip", this::pickArchive).help("只含音频、指令模板等数据，不含可执行内容，每个包须声明许可证");
        if (row.button() != null) row.button().setEnabled(!this.importing);
        if (!notice.isEmpty()) importing.note(notice);
    }

    private void pickArchive() {
        if (importing) return;
        openArchive.launch(ARCHIVE_TYPES);
    }

    private void onArchivePicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        importing = true;
        notice = "正在导入…";
        render();
        HostTask.run(this, context -> PluginStore.importArchive(context, uri), result -> {
            importing = false;
            if (result == null || !result.succeeded()) {
                notice = "";
                MsToast.show(requireContext(), result == null ? "导入失败，请重试。"
                    : PluginCatalogPolicy.errorMessage(result.code(), result.detail(), "导入失败，请重试。"));
            } else {
                PluginCatalogPolicy.Pack pack = result.value().pack();
                notice = PluginCatalogPolicy.importedNotice(pack)
                    + (PluginCatalogPolicy.usedOnPhone(pack) ? "" : "这类插件在这台设备上不生效，可以在电脑上使用。");
            }
            reload();
        });
    }

    // ---- 社区插件 ----

    private void renderCommunity(LinearLayout column) {
        Context context = requireContext();
        column.removeAllViews();
        SearchPill search = new SearchPill(context);
        search.setHint("搜索插件");
        search.field().setText(communitySearch);
        search.field().setImeOptions(EditorInfo.IME_ACTION_SEARCH);
        search.setOnQueryChange(query -> communitySearch = query);
        search.field().setOnEditorActionListener((view, action, event) -> {
            communityQuery = communitySearch.trim();
            loadCommunity(false);
            return true;
        });
        LinearLayout.LayoutParams searchParams = LayoutPolicy.matchWidthHeightDp(context, 48);
        searchParams.bottomMargin = DimensionPolicy.pixels(context, 4);
        column.addView(search, searchParams);

        GroupCard filters = GroupCard.add(column, null).withDividers(Ui.ROW_PADDING_H);
        // 这一组没有组标题，类型说明挂在「插件类型」这一行的「?」上。
        filters.nav("插件类型", null, communityKind == null ? "全部" : PluginCatalogPolicy.kindLabel(communityKind),
            this::pickCommunityKind)
            .help("音效、指令表、短语表、辅助码表、单词本与符号集，安装后在「我的插件」中选用。只列这台设备上能用的类型。");
        filters.nav("插件范围", null, communityMine ? "我的作品" : "全部插件", this::pickCommunityScope);
        filters.button("发布我的插件", null, "发布",
            () -> SettingsNavigator.open(context, PageId.PLUGIN_PUBLISH, null)).help("把自己导入或制作的插件分享到社区，要登录水杉账号");

        GroupCard list = GroupCard.add(column, null).withDividers(Ui.ROW_PADDING_H);
        if (!communityError.isEmpty()) {
            list.note(communityError);
            if (communitySignIn) list.button("登录水杉账号", null, "登录", this::signIn);
        }
        for (PluginStore.CommunityPlugin plugin : community) {
            list.nav(plugin.name(), communitySubtitle(plugin), communityMarker(plugin),
                () -> CommunityPluginPage.open(context, plugin.id()), true);
        }
        if (communityLoading) {
            list.note("正在读取插件…");
        } else if (communityLoaded && communityError.isEmpty() && community.isEmpty()) {
            list.note(communityHasMore
                ? (communityQuery.isEmpty() && communityKind == null
                    ? "前面的插件这台设备都不能安装，点「加载更多」查看更早发布的插件。"
                    : "前面的插件这台设备都不能安装，点「加载更多」继续查找。")
                : communityMine ? "你还没有发布过插件。"
                : !communityQuery.isEmpty() || communityKind != null ? "没有匹配的插件。"
                : "社区里还没有插件，安装或制作插件后可以点「发布我的插件」分享出来。");
        }
        if (communityHasMore && !communityLoading) list.button("更多插件", null, "加载更多", () -> loadCommunity(true));
        if (list.card().getChildCount() == 0) ViewPolicy.hide(list.view());
    }

    /** 列表行的副标题：类型、作者、评分和使用次数。 */
    private static String communitySubtitle(PluginStore.CommunityPlugin plugin) {
        StringBuilder text = new StringBuilder(PluginCatalogPolicy.kindLabel(plugin.kind()));
        if (!plugin.author().isEmpty()) text.append(" · ").append(plugin.author());
        if (plugin.ratingCount() > 0) {
            text.append(" · ★").append(String.format(java.util.Locale.ROOT, "%.1f", plugin.ratingAverage()))
                .append("（").append(plugin.ratingCount()).append("）");
        }
        text.append(" · 使用 ").append(plugin.downloads()).append(" 次");
        if (!plugin.description().isEmpty()) text.append('\n').append(plugin.description());
        return text.toString();
    }

    @Nullable private static String communityMarker(PluginStore.CommunityPlugin plugin) {
        if ("removed".equals(plugin.moderation())) return "已下架";
        return plugin.owned() ? "我的作品" : null;
    }

    private void pickCommunityKind() {
        OptionSheet sheet = new OptionSheet(requireContext(), "插件类型", null);
        sheet.option("全部", communityKind == null, () -> changeCommunityKind(null));
        for (String kind : PluginCatalogPolicy.PHONE_KINDS) {
            sheet.option(PluginCatalogPolicy.kindLabel(kind), kind.equals(communityKind), () -> changeCommunityKind(kind));
        }
        sheet.show();
    }

    private void changeCommunityKind(@Nullable String kind) {
        communityKind = kind;
        loadCommunity(false);
    }

    private void pickCommunityScope() {
        OptionSheet sheet = new OptionSheet(requireContext(), "插件范围", "我的作品要登录水杉账号，已下架的也会列出");
        sheet.option("全部插件", !communityMine, () -> {
            communityMine = false;
            loadCommunity(false);
        });
        sheet.option("我的作品", communityMine, () -> {
            communityMine = true;
            loadCommunity(false);
        });
        sheet.show();
    }

    private void signIn() {
        SignIn.start(requireActivity(), failure -> {
            if (getView() == null) return;
            if (failure != null && !failure.isEmpty() && !LoginSheet.CANCELLED.equals(failure)) {
                MsToast.show(requireContext(), failure);
            }
            loadCommunity(false);
        });
    }

    /** 读一页社区插件；`more` 为假时从头读，换掉现有列表。旧请求的回答按代次丢掉。 */
    private void loadCommunity(boolean more) {
        long generation = ++communityGeneration;
        communitySeen = communityRevision;
        int offset = more ? communityOffset : 0;
        String query = communityQuery;
        String kind = communityKind;
        boolean mine = communityMine;
        communityLoading = true;
        if (!more) {
            communityError = "";
            communitySignIn = false;
        }
        rerenderCommunity();
        HostTask.runNetwork(this, context -> PluginStore.communityList(context, offset, query, kind, mine), result -> {
            if (generation != communityGeneration) return;
            communityLoading = false;
            communityLoaded = true;
            if (result == null || !result.succeeded()) {
                String code = result == null ? "" : result.code();
                communityError = PluginCatalogPolicy.communityMessage(code, result == null ? "" : result.detail(), false);
                communitySignIn = PluginCatalogPolicy.needsSignIn(code);
            } else {
                PluginStore.CommunityPage page = result.value();
                if (!more) community.clear();
                community.addAll(page.plugins());
                communityOffset = offset + page.plugins().size() + page.skipped();
                communityHasMore = page.hasMore();
                communityError = "";
                communitySignIn = false;
            }
            rerenderCommunity();
        });
    }

    /** 只重画社区插件这一类；整页还没画过时画整页，保证两类的先后顺序。 */
    private void rerenderCommunity() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        if (target.getChildCount() == 0) {
            render();
            return;
        }
        renderCommunity(sectionColumn(target, COMMUNITY));
    }

    @Override protected void onContentDestroyed() {
        // 视图没了，在途的社区请求不会再送回来（HostTask 只在视图还在时交付）：丢掉它，回来时按需重读。
        communityGeneration++;
        communityLoading = false;
        // 离开再回来时，导入提示已经过时。
        notice = "";
    }

    /** 社区插件页装好、下架或发布之后调用：回到插件页时社区列表从头读一次，我的插件本来就每次重读。 */
    static void communityChanged() {
        communityRevision++;
    }
}
