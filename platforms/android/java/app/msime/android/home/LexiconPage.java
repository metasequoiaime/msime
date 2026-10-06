package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.InputFeatureToggle;
import app.msime.android.HttpBodyPolicy;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 词库页：标题行的「刷新 / 导出 / 导入」，已安装（内置拼音词库加各个命名词库），新建与导入，发现词库（社区），学习组「记忆新词」，以及只在 Tauri 合包里出现的「更多」组（背单词、云词库，P21）。
 *
 * <p>命名词库的元数据和词条都经 {@link DictionaryCollectionsStore}；导入和导出走系统文件选择器（SAF），不申请存储权限。导入来源对话框只列 client-core 实际接受的格式。
 */
public final class LexiconPage extends DetailPage {
    private static final String BUILTIN_KIND = "pinyin";
    private static final int DISCOVER_LIMIT = 8;

    /** 渲染一次需要的全部数据；`builtinCount` 为负表示读不到。 */
    private record Model(DictionaryCollectionsStore.View view, long builtinCount, boolean learning, String failure) {}

    @Nullable private LinearLayout column;
    @Nullable private Model model;
    @Nullable private List<CommunityCatalog.Item> discover;
    @Nullable private String discoverFailure;
    /** 正在进行的导入所选的格式；文件名推不出格式时用它。 */
    private String pendingFormat = "txt";
    private final Set<String> installing = new HashSet<>();

    private final ActivityResultLauncher<String[]> openDocument =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onImportPicked);
    private final ActivityResultLauncher<String> createDocument =
        registerForActivityResult(new ActivityResultContracts.CreateDocument("text/plain"), this::onExportPicked);

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        buildHeader();
        if (model != null) render();
        reload(false);
        loadDiscover();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload(false);
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void buildHeader() {
        LinearLayout actions = headerActions();
        actions.removeAllViews();
        actions.addView(headerPill("↻", "刷新", false, () -> reload(true)));
        actions.addView(headerPill("↦", "导出", false, this::startExport));
        actions.addView(headerPill("⇪", "导入", true, this::showImportSources));
    }

    private TextView headerPill(String glyph, String label, boolean filled, Runnable action) {
        Context context = requireContext();
        TextView pill = new TextView(context);
        pill.setText(glyph + " " + label);
        pill.setGravity(Gravity.CENTER);
        pill.setSingleLine(true);
        int fill = filled ? Ui.accent(context) : Ui.accentSoft(context);
        Ui.style(pill, Ui.TEXT_BUTTON_SMALL, 500, filled ? Ui.onAccent(context) : Ui.accent(context));
        pill.setBackground(Ui.pillRipple(context, fill));
        Ui.setSymmetricPaddingDp(pill, context, 12, 6);
        Ui.setTextMinHeightDp(pill, context, Ui.COMPACT_BUTTON_MIN_HEIGHT);
        pill.setClickable(true);
        pill.setFocusable(true);
        pill.setContentDescription(label);
        pill.setOnClickListener(ignored -> action.run());
        LinearLayout.LayoutParams params = Ui.wrap();
        params.setMarginStart(Ui.dp(context, 8));
        pill.setLayoutParams(params);
        return pill;
    }

    // ---- 数据 ----

    private void reload(boolean flush) {
        HostTask.run(this, context -> read(context, flush), result -> {
            if (result == null) {
                MsToast.show(requireContext(), DictionaryCollectionsStore.failureMessage(""));
                return;
            }
            model = result;
            render();
            if (flush) MsToast.show(requireContext(), result.failure().isEmpty() ? "已刷新" : result.failure());
        });
    }

    private static Model read(Context context, boolean flush) {
        DictionaryCollectionsStore.Result<DictionaryCollectionsStore.View> view = flush
            ? DictionaryCollectionsStore.flush(context) : DictionaryCollectionsStore.load(context);
        DictionaryCollectionsStore.Result<Long> count = DictionaryCollectionsStore.builtinCount(context, BUILTIN_KIND);
        JSONObject snapshot = HostStore.loadPreferences(context);
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
        boolean learning = preferences == null
            ? InputFeatureToggle.LEARNING.enabledByDefault()
            : preferences.optBoolean(InputFeatureToggle.LEARNING.key(), InputFeatureToggle.LEARNING.enabledByDefault());
        DictionaryCollectionsStore.View value = view.ok() ? view.value()
            : new DictionaryCollectionsStore.View(List.of(), List.of(), null);
        return new Model(value, count.ok() ? count.value() : -1, learning, view.failure());
    }

    private void loadDiscover() {
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).list(CommunityRequest.Kind.DICTIONARY, "", 0, null),
            page -> {
                if (page == null || page.failed()) {
                    discover = null;
                    discoverFailure = page == null ? "暂时连不上社区，稍后再试。" : page.failure();
                } else {
                    List<CommunityCatalog.Item> items = page.items();
                    discover = CommunityRequest.limitedCopy(items, DISCOVER_LIMIT);
                    discoverFailure = null;
                }
                render();
            });
    }

    // ---- 渲染 ----

    private void render() {
        LinearLayout target = column;
        Model current = model;
        if (target == null || current == null) return;
        Context context = requireContext();
        target.removeAllViews();

        GroupCard installed = GroupCard.add(target, "已安装").withDividers(58);
        String builtinCount = current.builtinCount() < 0 ? null
            : DictionaryCollectionsStore.countLabel(current.builtinCount());
        installed.addView(KeyboardSheets.badgeNavRow(context, "汉", "拼音词库", builtinCount, "已启用",
            Ui.accent(context), () -> openDetail(DictionaryCollectionsStore.BUILTIN_PINYIN, "拼音词库")));
        for (DictionaryCollectionsStore.Collection collection : current.view().collections()) {
            String subtitle = DictionaryCollectionsStore.countLabel(collection.entryCount())
                + ("community".equals(collection.sourceType()) ? " · 社区" : "");
            installed.addView(KeyboardSheets.badgeNavRow(context, initial(collection.name()), collection.name(),
                subtitle, collection.enabled() ? "已启用" : "已停用",
                collection.enabled() ? Ui.accent(context) : Ui.subText(context),
                () -> openDetail(collection.id(), collection.name())));
        }
        installed.footer("点进词库可以启用、停用和编辑词条。已启用的词库会一起参与候选。");
        if (!current.failure().isEmpty()) installed.note(current.failure());

        GroupCard manage = GroupCard.add(target, null).withDividers(58);
        manage.addView(KeyboardSheets.actionRow(context, "+", "新建词库", this::showCreateDialog,
            28, 0, Ui.ROW_GAP));
        manage.addView(KeyboardSheets.actionRow(context, "⇪", "导入词库", this::showImportSources,
            28, 0, Ui.ROW_GAP));

        GroupCard community = GroupCard.add(target, "发现词库").withDividers(58);
        List<CommunityCatalog.Item> items = discover;
        if (items == null) {
            community.note(discoverFailure == null ? "正在读取社区词库…" : discoverFailure);
        } else if (items.isEmpty()) {
            community.note("社区里还没有词库。");
        } else {
            for (CommunityCatalog.Item item : items) community.addView(discoverRow(item, current.view()));
        }

        GroupCard learning = GroupCard.add(target, "学习");
        InputFeatureToggle toggle = InputFeatureToggle.LEARNING;
        learning.toggle(toggle.title(), toggle.description(), current.learning(), this::saveLearning);

        if (tauriAvailable()) {
            GroupCard more = GroupCard.add(target, "更多");
            more.nav("背单词", "在管理界面里复习收藏的单词", null, this::openVocabularyReview);
            more.nav("云词库", "在管理界面里管理云端词库", null, this::openCloudDictionary);
        }
    }

    private View discoverRow(CommunityCatalog.Item item, DictionaryCollectionsStore.View view) {
        Context context = requireContext();
        LinearLayout row = KeyboardSheets.baseRow(context);
        row.addView(KeyboardSheets.badge(context, initial(item.name())));
        List<String> parts = new ArrayList<>(2);
        if (!item.author().isEmpty()) parts.add("@" + item.author());
        JSONArray words = item.payload() == null ? null : item.payload().optJSONArray("words");
        if (words != null) parts.add(DictionaryCollectionsStore.countLabel(words.length()));
        if (parts.isEmpty() && !item.description().isEmpty()) parts.add(item.description());
        row.addView(KeyboardSheets.texts(context, item.name(), parts.isEmpty() ? null : String.join(" · ", parts),
                Ui.text(context)),
            Ui.weightWrap(1f));
        boolean added = view.installed(item.id());
        boolean busy = installing.contains(item.id());
        TextView button = new TextView(context);
        button.setText(added ? "已添加" : busy ? "添加中" : "添加");
        button.setGravity(Gravity.CENTER);
        button.setSingleLine(true);
        Ui.style(button, Ui.TEXT_BUTTON_SMALL, 500, added ? Ui.subText(context) : Ui.accent(context));
        button.setBackground(Ui.pillRipple(context, added ? Ui.rowBackground(context) : Ui.accentSoft(context)));
        Ui.setButtonPadding(button, context);
        Ui.setTextMinHeightDp(button, context, Ui.COMPACT_BUTTON_MIN_HEIGHT);
        boolean enabled = !added && !busy;
        button.setEnabled(enabled);
        button.setClickable(enabled);
        button.setFocusable(enabled);
        if (enabled) button.setOnClickListener(ignored -> install(item));
        button.setAccessibilityDelegate(KeyboardSheets.buttonDelegate(button.getText() + "，" + item.name()));
        LinearLayout.LayoutParams params = Ui.wrap();
        params.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
        row.addView(button, params);
        return row;
    }

    private static String initial(String name) {
        if (name == null || name.isEmpty()) return "词";
        return new String(Character.toChars(name.codePointAt(0)));
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
        List<String> formats = current == null ? List.of() : current.view().formats();
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
        if (report == null) return "已导入「" + name + "」";
        StringBuilder message = new StringBuilder("已导入「").append(name).append("」，")
            .append(DictionaryCollectionsStore.countLabel(report.imported()));
        if (report.duplicates() > 0) message.append("，跳过重复 ").append(report.duplicates()).append(" 条");
        if (report.failed() > 0) message.append("，").append(report.failed()).append(" 行无法识别");
        if (report.truncated()) message.append("，词库已满");
        return message.toString();
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
            output.write(text.value().getBytes(StandardCharsets.UTF_8));
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

    private void openVocabularyReview() {
        if (!tauriAvailable()) {
            MsToast.show(requireContext(), "背单词需要管理界面合包，请使用 Tauri 合包打开。");
            return;
        }
        Intent intent = new Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "vocabulary");
        startActivity(intent);
    }

    private void openCloudDictionary() {
        if (!tauriAvailable()) {
            MsToast.show(requireContext(), "云词库需要管理界面合包，请使用 Tauri 合包打开。您仍可在本机使用词库设置。");
            return;
        }
        Intent intent = new Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_mobile_panel", "cloud-dictionary");
        startActivity(intent);
    }

    /** 独立的原生 APK 没有 WebView 管理界面，Tauri 合包才有。 */
    private static boolean tauriAvailable() {
        try {
            Class.forName("app.msime.android.MainActivity");
            return true;
        } catch (ClassNotFoundException missing) {
            return false;
        }
    }
}
