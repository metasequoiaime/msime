package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DrawablePolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.ClearLearningCopy;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.InputFeatureToggle;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.ListPolicy;
import app.msime.android.NumberPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 词库页：已安装（内置拼音词库加各个命名词库，操作收在标题右侧的管理菜单），发现词库（社区里的前几个，末尾「浏览更多词库」进社区词库的完整列表），学习组「记忆新词」、「背单词」（原生页 {@link VocabularyReviewPage}）和「清除学习记录」，以及只在 Tauri 合包里出现的「更多」组（云词库，P21）。
 *
 * <p>新建、导入、导出和刷新是低频管理操作，放进「已安装」标题右侧的菜单，不在词库列表下面占四行高度；大标题和其他详情页一样只放标题。
 *
 * <p>命名词库的元数据和词条都经 {@link DictionaryCollectionsStore}；导入和导出走系统文件选择器（SAF），不申请存储权限。导入来源对话框只列 client-services 实际接受的格式。
 */
public final class LexiconPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("已安装", "社区", "学习");
    }

    private static final String BUILTIN_KIND = "pinyin";
    private static final int DISCOVER_LIMIT = 8;

    /** 渲染一次需要的全部数据；`builtinCount` 为负表示读不到。 */
    private record Model(DictionaryCollectionsStore.View view, long builtinCount, boolean learning, String failure) {}

    @Nullable private Model model;
    @Nullable private JSONObject updateStatus;
    private int updateGeneration;
    @Nullable private List<CommunityCatalog.Item> discover;
    @Nullable private String discoverFailure;
    /** 正在进行的导入所选的格式；文件名推不出格式时用它。 */
    private String pendingFormat = "txt";
    private final Set<String> installing = new HashSet<>();
    /** 正在清除学习记录：期间再点不再弹确认。 */
    private boolean clearing;

    private final ActivityResultLauncher<String[]> openDocument =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onImportPicked);
    private final ActivityResultLauncher<String> createDocument =
        registerForActivityResult(new ActivityResultContracts.CreateDocument("text/plain"), this::onExportPicked);

    @Override protected void onContentReady() {
        if (model != null) render();
    }

    @Override protected void onInitialReloadScheduled() {
        updateDictionary("status", null);
        loadDiscover();
    }

    @Override protected void reload() {
        reload(false);
    }

    @Override protected void onContentDestroyed() {
        updateGeneration++;
        installing.clear();
    }

    // ---- 数据 ----

    private void reload(boolean flush) {
        HostTask.run(this, LexiconPage::read, result -> {
            if (result == null) {
                MsToast.show(requireContext(), DictionaryCollectionsStore.failureMessage(""));
                if (model == null) renderUnavailable();
                return;
            }
            model = result;
            render();
            if (flush) MsToast.show(requireContext(), result.failure().isEmpty() ? "已刷新" : result.failure());
        });
    }

    private void updateDictionary(String command, Boolean enabled) {
        final int generation = ++updateGeneration;
        HostTask.run(this, context -> DictionaryUpdates.request(context, command, enabled), result -> {
            if (generation != updateGeneration) return;
            updateStatus = result;
            if (model != null) render();
            if (result != null && ("checking".equals(result.optString("phase")) || "downloading".equals(result.optString("phase")))) {
                View view = getView();
                if (view != null) view.postDelayed(() -> {
                    if (getView() == view && generation == updateGeneration) updateDictionary("status", null);
                }, 2000);
            }
        });
    }

    private static Model read(Context context) {
        // 打开页面也顺手送一批导入词库的待写入词条：返回的视图和 load 一样，只是多送了一批，不必等用户点刷新。
        DictionaryCollectionsStore.Result<DictionaryCollectionsStore.View> view = DictionaryCollectionsStore.flush(context);
        DictionaryCollectionsStore.Result<Long> count = DictionaryCollectionsStore.builtinCount(context, BUILTIN_KIND);
        JSONObject preferences = KeyboardSheets.preferences(context);
        boolean learning = preferences == null
            ? InputFeatureToggle.LEARNING.enabledByDefault()
            : preferences.optBoolean(InputFeatureToggle.LEARNING.key(), InputFeatureToggle.LEARNING.enabledByDefault());
        DictionaryCollectionsStore.View value = view.ok() ? view.value()
            : new DictionaryCollectionsStore.View(List.of(), List.of(), 0, null);
        return new Model(value, count.ok() ? count.value() : -1, learning, view.failure());
    }

    private void loadDiscover() {
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).list(CommunityRequest.Kind.DICTIONARY, "", 0, null),
            page -> {
                CommunityCatalog.Discovery result = CommunityCatalog.discovery(page, DISCOVER_LIMIT);
                discover = result.items();
                discoverFailure = result.failure();
                render();
            });
    }

    // ---- 渲染 ----

    private void render() {
        LinearLayout target = contentColumn();
        Model current = model;
        if (target == null || current == null) return;
        Context context = requireContext();
        target.removeAllViews();

        GroupCard installed = GroupCard.add(sectionColumn(target, "已安装"), "已安装").withDividers(58);
        installed.headerAction(manageButton(context));
        String builtinCount = current.builtinCount() < 0 ? null
            : NumberPolicy.groupedCount(current.builtinCount());
        installed.addView(KeyboardSheets.badgeNavRow(context, "汉", "拼音词库", builtinCount, "已启用",
            ThemeColorPolicy.accent(context), () -> openDetail(DictionaryCollectionsStore.BUILTIN_PINYIN, "拼音词库")));
        for (DictionaryCollectionsStore.Collection collection : current.view().collections()) {
            String subtitle = NumberPolicy.groupedCount(collection.entryCount())
                + DictionaryCollectionsStore.pendingDescription(collection.pending())
                + ("community".equals(collection.sourceType()) ? " · 社区" : "");
            installed.addView(KeyboardSheets.badgeNavRow(context, TextPolicy.initial(collection.name(), "词"), collection.name(),
                subtitle, collection.enabled() ? "已启用" : "已停用",
                collection.enabled() ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.subText(context),
                () -> openDetail(collection.id(), collection.name())));
        }
        installed.help("点进词库可以启用、停用和编辑词条。已启用的词库会一起参与候选。");
        if (!current.failure().isEmpty()) installed.note(current.failure());

        GroupCard updates = GroupCard.add(sectionColumn(target, "已安装"), "词库更新");
        updates.toggle("自动更新词库", null, updateStatus == null || updateStatus.optBoolean("enabled", true),
            enabled -> updateDictionary("configure", enabled)).help("每天检查兼容的词库版本");
        updates.note(DictionaryUpdates.message(updateStatus));
        boolean updating = updateStatus != null && ("checking".equals(updateStatus.optString("phase")) || "downloading".equals(updateStatus.optString("phase")));
        updates.nav("检查词库更新", null, null, () -> updateDictionary("check", null)).setEnabled(!updating);
        if (updateStatus != null && updateStatus.optBoolean("can_rollback")) {
            updates.nav("恢复上一版词库", "恢复后关闭自动更新，下次启动生效", null, () -> updateDictionary("rollback", null)).setEnabled(!updating);
        }

        GroupCard sync = GroupCard.add(sectionColumn(target, "已安装"), "同步");
        sync.nav(PageId.DICTIONARY_SYNC.title(), null, null,
            () -> SettingsNavigator.open(context, PageId.DICTIONARY_SYNC, null)).help("「我的词条」经自己的 WebDAV 网盘在设备之间同步");

        GroupCard community = GroupCard.add(sectionColumn(target, "社区"), "发现词库").withDividers(58);
        List<CommunityCatalog.Item> items = discover;
        if (items == null) {
            community.note(discoverFailure == null ? "正在读取社区词库…" : discoverFailure);
        } else if (items.isEmpty()) {
            community.note("社区里还没有词库。");
        } else {
            for (CommunityCatalog.Item item : items) community.addView(discoverRow(item, current.view()));
        }
        // 这里只列前几个，完整列表（搜索、详情、举报）压在词库页上面，返回回到这里。
        community.nav("浏览更多词库", null, null,
            () -> CommunityFragment.browse(context, CommunityRequest.Kind.DICTIONARY)).help("搜索社区里的全部词库");
        community.nav("提交缺失词条", null, null,
            () -> AboutPage.openLink(context, "https://msime.app/words/")).help("前往官网填写，审核后随公共词库更新；个人学习不会自动公开");

        GroupCard learning = GroupCard.add(sectionColumn(target, "学习"), "学习");
        InputFeatureToggle toggle = InputFeatureToggle.LEARNING;
        learning.toggle(toggle.title(), null, current.learning(), this::saveLearning).help(toggle.description());
        learning.nav(PageId.VOCABULARY_REVIEW.title(), null, null,
            () -> SettingsNavigator.open(context, PageId.VOCABULARY_REVIEW, null)).help("用内置词书或自己的词表复习英文单词");
        learning.button(ClearLearningCopy.TITLE, ClearLearningCopy.SUBTITLE, ClearLearningCopy.ACTION, this::confirmClearLearning);

        if (ManagementUi.available()) {
            GroupCard more = GroupCard.add(sectionColumn(target, "已安装"), "更多");
            more.nav("云词库", null, null, this::openCloudDictionary).help("在管理界面里管理云端词库");
        }
    }

    /** 已安装词库的低频管理动作：放在标题旁的菜单，避免四行操作卡片挤占列表空间。 */
    private View manageButton(Context context) {
        TextView button = HomeButtonPolicy.tonalButton(context, "管理", "词库管理", 500);
        ViewPolicy.bindClick(button, () -> showManageMenu(button));
        return button;
    }

    private void showManageMenu(View anchor) {
        PopupMenu menu = new PopupMenu(requireContext(), anchor);
        menu.getMenu().add("新建词库").setOnMenuItemClickListener(item -> {
            showCreateDialog();
            return true;
        });
        menu.getMenu().add("导入词库").setOnMenuItemClickListener(item -> {
            showImportSources();
            return true;
        });
        menu.getMenu().add("刷新词库").setOnMenuItemClickListener(item -> {
            reload(true);
            return true;
        });
        menu.getMenu().add("导出词库").setOnMenuItemClickListener(item -> {
            startExport();
            return true;
        });
        menu.show();
    }

    /** 第一次读就失败时页面上什么数据也没有；正常页面的刷新入口在管理菜单里，这里单独留一张卡片给它，不让页面空着没法重试。 */
    private void renderUnavailable() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        Context context = requireContext();
        target.removeAllViews();
        GroupCard card = GroupCard.add(target, null);
        card.note(DictionaryCollectionsStore.failureMessage(""));
        card.addView(KeyboardSheets.actionRow(context, "↻", "刷新词库", () -> reload(true),
            28, 0, Ui.ROW_GAP));
    }

    private View discoverRow(CommunityCatalog.Item item, DictionaryCollectionsStore.View view) {
        Context context = requireContext();
        LinearLayout row = KeyboardSheets.baseRow(context);
        row.addView(KeyboardSheets.badge(context, TextPolicy.initial(item.name(), "词")));
        List<String> parts = new ArrayList<>(2);
        if (!item.author().isEmpty()) parts.add("@" + item.author());
        JSONArray words = item.payload() == null ? null : item.payload().optJSONArray("words");
        if (words != null) parts.add(NumberPolicy.groupedCount(words.length()));
        if (parts.isEmpty() && !item.description().isEmpty()) parts.add(item.description());
        row.addView(KeyboardSheets.texts(context, item.name(), parts.isEmpty() ? null : String.join(" · ", parts),
                ThemeColorPolicy.text(context)),
            LayoutPolicy.weightedWrapParams(1f));
        boolean added = view.installed(item.id());
        boolean busy = installing.contains(item.id());
        boolean enabled = !added && !busy;
        TextView button;
        if (enabled) {
            button = ViewPolicy.pillButton(context, "添加", Ui.TEXT_BUTTON_SMALL, 500,
                ThemeColorPolicy.accentSoft(context), ThemeColorPolicy.accent(context), Ui.BUTTON_PADDING_H, Ui.BUTTON_PADDING_V,
                Ui.COMPACT_BUTTON_MIN_HEIGHT, 0, () -> install(item));
        } else {
            button = ViewPolicy.centeredSingleLineLabel(context, added ? "已添加" : "添加中",
                Ui.TEXT_BUTTON_SMALL, 500, ThemeColorPolicy.subText(context));
            ViewPolicy.setBackground(button, ViewPolicy.pillRipple(context,
                added ? ThemeColorPolicy.rowBackground(context) : ThemeColorPolicy.accentSoft(context)));
            ViewPolicy.setButtonPadding(context, button);
            ViewPolicy.setTextMinHeightDp(context, button, Ui.COMPACT_BUTTON_MIN_HEIGHT);
            ViewPolicy.setEnabled(button, false);
        }
        button.setAccessibilityDelegate(HomeButtonPolicy.buttonDelegate(button.getText() + "，" + item.name()));
        LinearLayout.LayoutParams params = LayoutPolicy.rowGapParams(context, Ui.ROW_GAP);
        row.addView(button, params);
        return row;
    }

    // ---- 操作 ----

    private void openDetail(String id, String name) {
        Bundle args = new Bundle();
        args.putString(LexiconDetailPage.ARG_ID, id);
        args.putString(LexiconDetailPage.ARG_NAME, name);
        SettingsNavigator.open(requireContext(), PageId.LEXICON_DETAIL, args);
    }

    private void showCreateDialog() {
        InputDialog dialog = new InputDialog(requireContext(), "新建词库", "给词库起个名字，比如「工作」「游戏」");
        dialog.addField("词库名称", null, 0);
        dialog.setValidator(values -> DictionaryCollectionsStore.validName(values.get(0)));
        dialog.setPrimary("创建", values -> create(values.get(0)));
        dialog.show();
    }

    private void create(String name) {
        Model current = model;
        Set<String> before = new HashSet<>(current == null ? 0 : current.view().collections().size());
        if (current != null) for (DictionaryCollectionsStore.Collection item : current.view().collections()) before.add(item.id());
        HostTask.run(this, context -> DictionaryCollectionsStore.create(context, name), result -> {
            if (result == null || !result.ok()) {
                MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
                return;
            }
            reload(false);
            for (DictionaryCollectionsStore.Collection item : result.value().collections()) {
                if (!before.contains(item.id())) {
                    openDetail(item.id(), item.name());
                    return;
                }
            }
        });
    }

    private void showImportSources() {
        Model current = model;
        List<String> formats = ListPolicy.copyOrEmpty(current == null ? null : current.view().formats());
        List<DictionaryCollectionsStore.ImportSource> sources = DictionaryCollectionsStore.importSources(formats);
        if (sources.isEmpty()) {
            MsToast.show(requireContext(), DictionaryCollectionsStore.failureMessage("unavailable"));
            return;
        }
        OptionSheet sheet = new OptionSheet(requireContext(), "导入来源", "导入的词条会成为一个新的词库");
        for (DictionaryCollectionsStore.ImportSource source : sources) {
            sheet.option(source.label(), false, () -> {
                pendingFormat = source.format();
                openDocument.launch(source.mimeTypes());
            });
        }
        sheet.show();
    }

    private void onImportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        String chosen = pendingFormat;
        HostTask.run(this, context -> importFrom(context, uri, chosen), message -> {
            MsToast.show(requireContext(), message == null ? DictionaryCollectionsStore.failureMessage("") : message);
            reload(false);
        });
    }

    /** 与共享设置页 `SCEL_PERSONAL_USE_NOTICE` 相同。 */
    static final String SCEL_PERSONAL_USE_NOTICE =
        "搜狗细胞词库的整理成果属于原作者，导入的词只供你在本机使用，不能发布到社区。";

    private static String importFrom(Context context, Uri uri, String chosen) {
        String displayName = displayName(context, uri);
        byte[] bytes;
        try {
            bytes = readAll(context, uri);
        } catch (IOException | SecurityException error) {
            return "读取文件失败，请重新选择。";
        }
        if (bytes == null) return DictionaryCollectionsStore.failureMessage("collections_too_large");
        String format = DictionaryCollectionsStore.formatForFile(displayName, chosen);
        String name = DictionaryCollectionsStore.nameFromFile(displayName);
        DictionaryCollectionsStore.Result<DictionaryCollectionsStore.View> result =
            DictionaryCollectionsStore.importFile(context, format, name, bytes);
        if (!result.ok()) return result.failure();
        DictionaryCollectionsStore.ImportReport report = result.value().importReport();
        // 细胞词库导入后说明只供本机使用：第三方词库的整理成果属于原作者，社区发布会拦下这些词（.agents/notes/implemented/feature/2026-10-17-scel-community-publish-guard.md）。
        String personalUse = "scel".equals(format) ? "。" + SCEL_PERSONAL_USE_NOTICE : "";
        if (report == null) return "已导入「" + name + "」" + personalUse;
        StringBuilder message = new StringBuilder("已导入「").append(name).append("」，")
            .append(NumberPolicy.groupedCount(report.imported()));
        if (report.duplicates() > 0) message.append("，跳过重复 ").append(report.duplicates()).append(" 条");
        if (report.failed() > 0) message.append("，").append(report.failed()).append(" 行无法识别");
        if (report.truncated()) message.append("，词库已满");
        if (!report.applicationNotice().isEmpty()) message.append("；").append(report.applicationNotice());
        return message.append(personalUse).toString();
    }

    /** 读完整个文件；超过导入上限时返回 null，不把超大文件整个读进内存。 */
    @Nullable private static byte[] readAll(Context context, Uri uri) throws IOException {
        try (InputStream input = context.getContentResolver().openInputStream(uri)) {
            if (input == null) throw new IOException("unreadable");
            return HttpBodyPolicy.readBounded(input, DictionaryCollectionsStore.MAX_IMPORT_BYTES);
        }
    }

    static String displayName(Context context, Uri uri) {
        try (Cursor cursor = context.getContentResolver().query(uri,
                new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String name = cursor.getString(0);
                if (name != null) return name;
            }
        } catch (RuntimeException ignored) {
            // 拿不到文件名时按内容格式导入，名字用默认值。
        }
        String path = uri.getLastPathSegment();
        return path == null ? "" : path;
    }

    private void startExport() {
        createDocument.launch("水杉词库.txt");
    }

    private void onExportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        HostTask.run(this, context -> exportTo(context, uri, BUILTIN_KIND), message ->
            MsToast.show(requireContext(), message == null ? DictionaryCollectionsStore.failureMessage("") : message));
    }

    /** 把一个词库种类导出到用户选的文件，返回要展示的结果。词库详情页也用它。 */
    static String exportTo(Context context, Uri uri, String kind) {
        DictionaryCollectionsStore.Result<String> text = DictionaryCollectionsStore.export(context, kind);
        if (!text.ok()) return text.failure();
        try (OutputStream output = context.getContentResolver().openOutputStream(uri, "wt")) {
            if (output == null) return "写入文件失败，请重新选择位置。";
            output.write(TextPolicy.utf8Bytes(text.value()));
        } catch (IOException | SecurityException error) {
            return "写入文件失败，请重新选择位置。";
        }
        return "已导出";
    }

    private void install(CommunityCatalog.Item item) {
        installing.add(item.id());
        render();
        HostTask.run(this, context -> DictionaryCollectionsStore.installCommunity(context, item.raw()), result -> {
            installing.remove(item.id());
            if (result == null || !result.ok()) {
                MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
                render();
                return;
            }
            MsToast.show(requireContext(), "已添加「" + item.name() + "」");
            reload(false);
        });
    }

    private void confirmClearLearning() {
        if (clearing) return;
        MessageDialog.confirm(requireContext(), ClearLearningCopy.CONFIRM_TITLE, ClearLearningCopy.CONFIRM_MESSAGE,
            ClearLearningCopy.ACTION, this::clearLearning);
    }

    private void clearLearning() {
        if (clearing) return;
        clearing = true;
        HostTask.run(this, DictionaryCollectionsStore::resetLearning, result -> {
            clearing = false;
            if (result == null || !result.ok()) {
                MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.resetFailureMessage("") : result.failure());
                return;
            }
            DictionaryCollectionsStore.ResetReport report = result.value();
            MsToast.show(requireContext(), ClearLearningCopy.cleared(report.pending(), report.collectionsFailed(), report.unsynced()));
            reload(false);
        });
    }

    private void saveLearning(boolean enabled) {
        String key = InputFeatureToggle.LEARNING.key();
        HostTask.run(this, context -> HostStore.putPreference(context, key, enabled), saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload(false);
            }
        });
    }

    // ---- 只在 Tauri 合包里有用的入口（P21），跳转写法与原 KeyboardFragment / AccountFragment 一致 ----

    private void openCloudDictionary() {
        if (!ManagementUi.available()) {
            MsToast.show(requireContext(), "云词库需要管理界面合包，请使用 Tauri 合包打开。您仍可在本机使用词库设置。");
            return;
        }
        startActivity(ManagementUi.mobilePanel(requireContext(), "cloud-dictionary"));
    }

}
