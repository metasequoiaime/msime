package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.MainThreadPolicy;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.NumberPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 词库详情：启用开关、搜索词条、词条列表（词、带撇号的拼音、权重）、添加词条和导出。
 *
 * <p>参数是 {@link #ARG_ID}（集合 id，内置拼音词库是 {@link DictionaryCollectionsStore#BUILTIN_PINYIN}，粤语词库是 {@link DictionaryCollectionsStore#BUILTIN_CANTONESE}）和 {@link #ARG_NAME}（标题）。粤语词库只有用户自己加的粤语词，编码是以空格分隔的粤拼。内置词库常开，开关置灰；它的新词经个人词库队列写入，因为键盘活着时直接编辑会返回 busy，键盘下次启动时应用。命名词库的启用、停用和加词由 client-services 分批送进同一个队列。
 *
 * <p>集合接口没有列出单个集合词条的操作，所以命名词库只显示条数，搜索和导出只对内置词库提供。
 */
public final class LexiconDetailPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("词条", "管理");
    }

    public static final String ARG_ID = "collection_id";
    public static final String ARG_NAME = "name";
    private static final int PAGE_SIZE = 100;
    private static final long SEARCH_DELAY_MILLIS = 250;

    /** 内置词库时 `collection` 为空、`count` 是总条数；命名词库时 `collection` 非空。 */
    private record Model(@Nullable DictionaryCollectionsStore.Collection collection, long count,
                         List<DictionaryCollectionsStore.Word> words, String failure) {}

    private final Handler main = MainThreadPolicy.mainHandler();
    private final ActivityResultLauncher<String> createDocument =
        registerForActivityResult(new ActivityResultContracts.CreateDocument("text/plain"), this::onExportPicked);
    @Nullable private GroupCard entries;
    @Nullable private Model model;
    private List<DictionaryCollectionsStore.Word> shown = new ArrayList<>(PAGE_SIZE);
    private String query = "";
    private int searchGeneration;

    @Override protected CharSequence title() {
        Bundle args = getArguments();
        String name = args == null ? null : args.getString(ARG_NAME);
        return name == null || name.isEmpty() ? "词库" : name;
    }

    @Override protected boolean shouldReloadWhenVisible() {
        return query.isEmpty();
    }

    @Override protected void onContentDestroyed() {
        main.removeCallbacksAndMessages(null);
        entries = null;
    }

    private String collectionId() {
        Bundle args = getArguments();
        String id = args == null ? null : args.getString(ARG_ID);
        return id == null || id.isEmpty() ? DictionaryCollectionsStore.BUILTIN_PINYIN : id;
    }

    private boolean builtin() {
        return collectionId().startsWith(DictionaryCollectionsStore.BUILTIN_PREFIX);
    }

    /** 内置词库对应的个人词库种类：粤语词库是 `cantonese`，其余（拼音词库）是 `pinyin`。 */
    private static String builtinKind(String id) {
        return DictionaryCollectionsStore.BUILTIN_CANTONESE.equals(id) ? "cantonese" : "pinyin";
    }

    private boolean cantonese() {
        return "cantonese".equals(builtinKind(collectionId()));
    }

    // ---- 数据 ----

    @Override protected void reload() {
        String id = collectionId();
        boolean builtin = builtin();
        HostTask.run(this, context -> read(context, id, builtin), result -> {
            if (result == null) {
                MsToast.show(requireContext(), DictionaryCollectionsStore.failureMessage(""));
                return;
            }
            model = result;
            shown = new ArrayList<>(result.words());
            render();
        });
    }

    private static Model read(Context context, String id, boolean builtin) {
        if (builtin) {
            String kind = builtinKind(id);
            DictionaryCollectionsStore.Result<Long> count = DictionaryCollectionsStore.builtinCount(context, kind);
            DictionaryCollectionsStore.Result<DictionaryCollectionsStore.WordPage> page =
                DictionaryCollectionsStore.words(context, kind, "", 0, PAGE_SIZE);
            return new Model(null, count.ok() ? count.value() : -1,
                page.ok() ? page.value().words() : List.of(), page.ok() ? "" : page.failure());
        }
        // 和词库列表页一样，打开时顺手送一批待写入的词条，显示的剩余条数也就是最新的。
        DictionaryCollectionsStore.Result<DictionaryCollectionsStore.View> view = DictionaryCollectionsStore.flush(context);
        if (!view.ok()) return new Model(null, -1, List.of(), view.failure());
        DictionaryCollectionsStore.Collection collection = view.value().find(id);
        if (collection == null) return new Model(null, -1, List.of(), DictionaryCollectionsStore.failureMessage("collections_not_found"));
        return new Model(collection, collection.entryCount(), List.of(), "");
    }

    // ---- 渲染 ----

    private void render() {
        LinearLayout target = contentColumn();
        Model current = model;
        if (target == null || current == null) return;
        target.removeAllViews();
        boolean builtin = builtin();
        DictionaryCollectionsStore.Collection collection = current.collection();
        if (!builtin && collection == null) {
            GroupCard.add(target, null).note(current.failure());
            return;
        }
        if (collection != null) setTitle(collection.name());

        GroupCard header = GroupCard.add(sectionColumn(target, "管理"), null);
        String count = current.count() < 0 ? null : NumberPolicy.groupedCount(current.count());
        String collectionDescription = collection == null ? null
            : count + DictionaryCollectionsStore.pendingDescription(collection.pending());
        String always = cantonese() ? "粤语方案始终启用" : "内置词库始终启用";
        GroupCard.Row toggle = header.toggle("启用此词库", builtin ? (count == null ? always : count + " · " + always) : collectionDescription,
            builtin || collection.enabled(), this::setEnabled);
        if (builtin) toggle.setEnabled(false);

        if (builtin) {
            SearchPill search = new SearchPill(requireContext());
            search.setHint("搜索词条");
            search.field().setText(query);
            search.setOnQueryChange(this::onQuery);
            LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
            params.topMargin = DimensionPolicy.pixels(requireContext(), Ui.GROUP_GAP);
            sectionColumn(target, "管理").addView(search, params);
        }

        entries = GroupCard.add(sectionColumn(target, "词条"), null).withDividers(0);
        renderEntries();

        if (builtin) {
            GroupCard export = GroupCard.add(sectionColumn(target, "管理"), null);
            export.addView(accentRow(null, "导出词库", () -> createDocument.launch(cantonese() ? "粤语词库.txt" : "拼音词库.txt")));
        } else {
            GroupCard manage = GroupCard.add(sectionColumn(target, "管理"), null);
            manage.addView(accentRow(null, "删除此词库", this::confirmDelete));
        }
    }

    private void renderEntries() {
        GroupCard card = entries;
        Model current = model;
        if (card == null || current == null) return;
        card.card().removeAllViews();
        if (builtin()) {
            if (!current.failure().isEmpty() && query.isEmpty()) {
                card.note(current.failure());
            } else if (shown.isEmpty()) {
                card.note(!query.isEmpty() ? "没有找到相关词条。" : cantonese()
                    ? "还没有自己加的粤语词。加进来的词在粤语方案里打同样的粤拼时出现。"
                    : "还没有自己添加或学到的词。输入拼音可以搜索内置词条。");
            }
            for (DictionaryCollectionsStore.Word word : shown) {
                card.value(word.value(), cantonese() ? word.key() : DictionaryCollectionsStore.displayCode(word.key()),
                    String.format(Locale.ROOT, "%d", word.weight()));
            }
        } else {
            DictionaryCollectionsStore.Collection collection = current.collection();
            if (collection != null) {
                String total = "这个词库有 " + NumberPolicy.groupedCount(collection.entryCount());
                // 待写入的词由键盘在每次收起后自动分批写完（MSIMEInputService 的空闲同步），这里只告诉用户正在进行、不用做什么。
                card.note(collection.pending() > 0
                    ? total + "，其中 " + collection.pending() + " 条正在写入键盘。每次用完键盘、收起后会自动接着写，不需要手动操作。"
                    : total + "。新加的词在下次打开键盘时生效。");
            }
        }
        card.addView(accentRow("+", "添加词条", this::showAddDialog));
    }

    private View accentRow(@Nullable String glyph, String title, Runnable action) {
        Context context = requireContext();
        return KeyboardSheets.accentActionRow(context, glyph, title, action, 24, 10, 0);
    }

    // ---- 搜索 ----

    private void onQuery(String text) {
        query = text;
        int generation = ++searchGeneration;
        main.removeCallbacksAndMessages(null);
        main.postDelayed(() -> search(generation, text), SEARCH_DELAY_MILLIS);
    }

    /** 拼音按编码前缀问 Engine（连内置词一起）；粤语词库按粤拼前缀查自己的粤语词；汉字在当前列表里按包含过滤；清空时回到自己的词。 */
    private void search(int generation, String text) {
        Model current = model;
        if (current == null || contentColumn() == null) return;
        boolean cantonese = cantonese();
        String kind = builtinKind(collectionId());
        String code = cantonese ? DictionaryCollectionsStore.normalizeCantonese(text)
            : DictionaryCollectionsStore.normalizePinyin(text).replace("'", "");
        if (text.isEmpty()) {
            shown = new ArrayList<>(current.words());
            renderEntries();
            return;
        }
        if (cantonese ? !DictionaryCollectionsStore.validCantonese(code) : !DictionaryCollectionsStore.validPinyin(code)) {
            List<DictionaryCollectionsStore.Word> filtered = new ArrayList<>(current.words().size());
            for (DictionaryCollectionsStore.Word word : current.words()) {
                if (word.value().contains(text)) filtered.add(word);
            }
            shown = filtered;
            renderEntries();
            return;
        }
        HostTask.run(this, context -> DictionaryCollectionsStore.words(context, kind, code, 0, PAGE_SIZE), page -> {
            if (generation != searchGeneration) return;
            shown = page != null && page.ok() ? new ArrayList<>(page.value().words()) : new ArrayList<>();
            renderEntries();
        });
    }

    // ---- 操作 ----

    private void setEnabled(boolean enabled) {
        Model current = model;
        if (current == null || current.collection() == null) return;
        String id = current.collection().id();
        HostTask.run(this, context -> DictionaryCollectionsStore.setEnabled(context, id, enabled), result -> {
            if (result == null || !result.ok()) {
                MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
            } else {
                MsToast.show(requireContext(), enabled ? "已启用，键盘下次启动时生效" : "已停用");
            }
            reload();
        });
    }

    private void showAddDialog() {
        InputDialog dialog = new InputDialog(requireContext(), "添加到「" + title() + "」", null);
        dialog.addField("词语", null, 0);
        if (builtin() && cantonese()) {
            // 粤拼每个字一个音节，用空格分开；带声调数字也行，Engine 存之前去掉。
            dialog.addField("粤拼，例如 nei hou", null, android.text.InputType.TYPE_CLASS_TEXT
                | android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD);
            dialog.setValidator(values -> !values.get(0).isEmpty()
                && DictionaryCollectionsStore.validCantonese(DictionaryCollectionsStore.normalizeCantonese(values.get(1))));
            dialog.setPrimary("添加", values -> add(values.get(0), DictionaryCollectionsStore.normalizeCantonese(values.get(1))));
            dialog.show();
            return;
        }
        dialog.addField("拼音，例如 shui'shan", null, android.text.InputType.TYPE_CLASS_TEXT
            | android.text.InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD);
        dialog.setValidator(values -> !values.get(0).isEmpty()
            && DictionaryCollectionsStore.validPinyin(DictionaryCollectionsStore.normalizePinyin(values.get(1))));
        dialog.setPrimary("添加", values -> add(values.get(0), DictionaryCollectionsStore.normalizePinyin(values.get(1))));
        dialog.show();
    }

    private void add(String value, String code) {
        Model current = model;
        if (current == null) return;
        DictionaryCollectionsStore.Collection collection = current.collection();
        if (collection == null) {
            boolean cantonese = cantonese();
            DictionaryCollectionsStore.Word word = new DictionaryCollectionsStore.Word(builtinKind(collectionId()), code, value,
                cantonese ? DictionaryCollectionsStore.CANTONESE_DEFAULT_WEIGHT : DictionaryCollectionsStore.DEFAULT_WEIGHT, "user");
            HostTask.run(this, context -> DictionaryCollectionsStore.queueWord(context, word), result -> {
                if (result == null || !result.ok()) {
                    MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
                    return;
                }
                MsToast.show(requireContext(), "已添加，键盘下次启动时生效");
                shown.add(0, word);
                renderEntries();
            });
            return;
        }
        DictionaryCollectionsStore.Word word = new DictionaryCollectionsStore.Word(collection.kind(), code, value,
            DictionaryCollectionsStore.DEFAULT_WEIGHT, "user");
        String id = collection.id();
        HostTask.run(this, context -> DictionaryCollectionsStore.addWords(context, id, List.of(word)), result -> {
            if (result == null || !result.ok()) {
                MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
                return;
            }
            MsToast.show(requireContext(), "已添加");
            reload();
        });
    }

    private void confirmDelete() {
        Model current = model;
        if (current == null || current.collection() == null) return;
        DictionaryCollectionsStore.Collection collection = current.collection();
        MessageDialog.confirm(requireContext(), "删除「" + collection.name() + "」？",
            "词库和它的词条会从本机删除；同时属于其他已启用词库的词会保留。", "删除", () -> HostTask.run(this,
                context -> DictionaryCollectionsStore.delete(context, collection.id()), result -> {
                    if (result == null || !result.ok()) {
                        MsToast.show(requireContext(), result == null ? DictionaryCollectionsStore.failureMessage("") : result.failure());
                        return;
                    }
                    MsToast.show(requireContext(), "已删除");
                    requireActivity().getOnBackPressedDispatcher().onBackPressed();
                }));
    }

    private void onExportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        String kind = builtinKind(collectionId());
        HostTask.run(this, context -> LexiconPage.exportTo(context, uri, kind), message ->
            MsToast.show(requireContext(), message == null ? DictionaryCollectionsStore.failureMessage("") : message));
    }
}
