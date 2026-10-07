package app.msime.android.home;

import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;

import android.content.Context;
import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.LinearLayout;
import android.widget.RadioButton;
import android.widget.RadioGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AlertDialog;
import androidx.fragment.app.Fragment;
import app.msime.android.BoundsPolicy;
import androidx.recyclerview.widget.GridLayoutManager;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.CommunitySkinCache;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.R;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.ViewPolicy;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;
import com.google.android.material.chip.Chip;
import com.google.android.material.chip.ChipGroup;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import com.google.android.material.textfield.TextInputEditText;
import com.google.android.material.textfield.TextInputLayout;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONObject;

/**
 * The 社区 tab: skins, dictionaries and phrase packs published by other people, with the reply templates as a second section under the phrase packs (「AI 回复模板」).
 *
 * <p>Publishing and rating need a signed-in account, and this host carries only the keyboard's anonymous identity; a publish button that always answers "请先登录" would be worse than the honest absence of one. Taking a work, which is what people open this tab to do, works on every card: a skin's 获取 saves it into the custom skin library (and counts a download), after which the same pill says 使用 and puts it on the keyboard; a dictionary's 添加 installs it as a named dictionary, a phrase pack's 添加 installs it into the common phrases. 例外是分类：登录了水杉账号的作者可以在详情里改自己皮肤的分类。
 */
public final class CommunityFragment extends Fragment {
    private static final String ARG_KIND = "kind";
    private static final String REPLY_SECTION = "AI 回复模板";

    /** The segment on screen: skins, dictionaries or phrases. */
    private CommunityRequest.Kind kind = CommunityRequest.Kind.SKIN;
    /** The kind being paged: equal to `kind`, except that the phrase segment pages its replies after its packs. */
    private CommunityRequest.Kind section = CommunityRequest.Kind.SKIN;
    private CommunityAdapter adapter;
    private String search = "";
    // 皮肤的分类筛选，null 是「全部」。只在皮肤页生效，词库和短语没有分类。
    @Nullable private CommunityRequest.Category category;
    private boolean loading;
    private boolean hasMore;
    // 预览按用户自己的布局画。读不到就按 26 键，那是默认值。
    private boolean nineKey;

    /** The tab, opened on one kind of work; a null kind opens on skins, reply templates open the phrase segment. */
    public static CommunityFragment forKind(@Nullable CommunityRequest.Kind kind) {
        CommunityFragment fragment = new CommunityFragment();
        if (kind != null) {
            Bundle arguments = new Bundle();
            arguments.putString(ARG_KIND, kind.id());
            fragment.setArguments(arguments);
        }
        return fragment;
    }

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_community, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        Bundle arguments = getArguments();
        if (arguments != null) {
            for (CommunityRequest.Kind value : CommunityRequest.kinds()) {
                if (value.id().equals(arguments.getString(ARG_KIND))) kind = value;
            }
        }
        // 回复模板没有自己的分段，它在「短语」分段的第二个小节里。
        if (kind == CommunityRequest.Kind.REPLY) kind = CommunityRequest.Kind.PHRASE;
        section = kind;

        // The design's outlined segmented control. The opening kind is checked before the listener is attached, so opening loads the listing once, below, rather than once per path.
        MaterialButtonToggleGroup kinds = view.findViewById(R.id.community_kinds);
        LayoutInflater inflater = LayoutInflater.from(requireContext());
        List<CommunityRequest.Kind> values = CommunityRequest.segments();
        int[] segments = new int[values.size()];
        for (int index = 0; index < values.size(); index++) {
            MaterialButton segment =
                (MaterialButton) inflater.inflate(R.layout.item_segment, kinds, false);
            segment.setId(View.generateViewId());
            segment.setText(values.get(index).title());
            kinds.addView(segment);
            segments[index] = segment.getId();
        }
        kinds.check(segments[BoundsPolicy.nonNegative(values.indexOf(kind))]);
        kinds.addOnButtonCheckedListener((group, id, checked) -> {
            if (!checked) return;
            for (int index = 0; index < segments.length; index++) {
                if (segments[index] != id || values.get(index) == kind) continue;
                kind = values.get(index);
                updateSearchHint();
                updateCategories();
                load(true);
            }
        });

        // 分类筛选条：「全部」在最前，后面是固定的八个分类。换分类和换搜索词一样从第一页重新载入。
        ChipGroup categories = view.findViewById(R.id.community_categories);
        List<CommunityRequest.Category> choices = CommunityRequest.categories();
        int[] chips = new int[choices.size() + 1];
        for (int index = 0; index < chips.length; index++) {
            Chip chip = new Chip(requireContext());
            chip.setId(View.generateViewId());
            chip.setText(index == 0 ? "全部" : choices.get(index - 1).label());
            chip.setCheckable(true);
            categories.addView(chip);
            chips[index] = chip.getId();
        }
        categories.check(chips[category == null ? 0 : choices.indexOf(category) + 1]);
        categories.setOnCheckedStateChangeListener((group, checked) -> {
            if (checked.isEmpty()) return;
            int id = checked.get(0);
            CommunityRequest.Category next = null;
            for (int index = 1; index < chips.length; index++) {
                if (chips[index] == id) next = choices.get(index - 1);
            }
            if (next == category) return;
            category = next;
            load(true);
        });
        updateCategories();

        adapter = new CommunityAdapter(this::open, this::act);
        HostTask.run(this, HostStore::loadPreferences, snapshot -> {
            // loadPreferences hands back the whole snapshot; the layout lives one level down.
            JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
            nineKey = preferences != null && "nine_key".equals(
                preferences.optString("touch_keyboard_layout", "twenty_six_key"));
            adapter.setNineKey(nineKey);
        });
        refreshTaken();
        RecyclerView items = view.findViewById(R.id.community_items);
        GridLayoutManager grid = new GridLayoutManager(requireContext(), CommunityAdapter.COLUMNS);
        grid.setSpanSizeLookup(new GridLayoutManager.SpanSizeLookup() {
            @Override public int getSpanSize(int position) { return adapter.span(position); }
        });
        items.setLayoutManager(grid);
        items.setAdapter(adapter);
        items.addOnScrollListener(new RecyclerView.OnScrollListener() {
            @Override public void onScrolled(@NonNull RecyclerView list, int dx, int dy) {
                if (dy <= 0 || loading || !hasMore) return;
                LinearLayoutManager manager = (LinearLayoutManager) list.getLayoutManager();
                if (manager == null) return;
                // One screen of slack so the next page is already arriving when the last card is reached, rather than after a stop at the bottom.
                if (manager.findLastVisibleItemPosition()
                    >= adapter.size() - CommunityRequest.PAGE_SIZE / 4) load(false);
            }
        });

        TextInputEditText field = view.findViewById(R.id.community_search);
        field.setOnEditorActionListener((text, action, event) -> {
            if (action != EditorInfo.IME_ACTION_SEARCH) return false;
            search = text.getText() == null ? "" : text.getText().toString();
            // The results are what the search was for, and the keyboard is sitting on top of them.
            android.view.inputmethod.InputMethodManager manager =
                requireContext().getSystemService(android.view.inputmethod.InputMethodManager.class);
            if (manager != null) manager.hideSoftInputFromWindow(text.getWindowToken(), 0);
            text.clearFocus();
            load(true);
            return true;
        });

        MaterialButton retry = view.findViewById(R.id.community_retry);
        ViewPolicy.bindClick(retry, () -> load(true));

        load(true);
        updateSearchHint();
        cacheSkinCatalogue();
    }

    /** 拉全社区皮肤目录写进本机缓存，键盘的皮肤面板据此列出全部社区皮肤（键盘进程不为浏览目录联网）。最多翻十页，失败时保留旧缓存。 */
    private void cacheSkinCatalogue() {
        HostTask.runNetwork(this, context -> {
            String directory = HostStore.directory(context);
            if (directory.isEmpty()) return null;
            CommunityCatalog catalog = new CommunityCatalog(context);
            java.util.List<CommunitySkinCache.Entry> entries = new java.util.ArrayList<>(
                10 * CommunityRequest.PAGE_SIZE);
            for (int page = 0; page < 10; page++) {
                CommunityCatalog.Page result = catalog.list(CommunityRequest.Kind.SKIN, "", entries.size(), null);
                if (result == null || result.failed()) return null;
                for (CommunityCatalog.Item item : result.items()) {
                    if (item.payload() != null) entries.add(new CommunitySkinCache.Entry(item.id(), item.name(), item.author(), item.payload()));
                }
                if (!result.hasMore() || result.items().isEmpty()) break;
            }
            try {
                CommunitySkinCache.write(Paths.get(directory), entries);
            } catch (java.io.IOException error) {
                android.util.Log.w("MSIMECommunity", "Skin catalogue cache was not written", error);
            }
            return null;
        }, ignored -> { });
    }

    private void updateSearchHint() {
        View view = getView();
        if (view == null) return;
        // The pill field has no floating label, so the hint lives on the text itself.
        ((TextInputEditText) view.findViewById(R.id.community_search)).setHint(
            kind == CommunityRequest.Kind.PHRASE ? "搜索短语和回复模板" : kind.searchHint());
    }

    /** 分类筛选条只属于皮肤页。 */
    private void updateCategories() {
        View view = getView();
        if (view == null) return;
        ViewPolicy.setVisible(view.findViewById(R.id.community_categories_scroll),
            kind == CommunityRequest.Kind.SKIN);
    }

    /** 本次请求实际用的分类：只有皮肤按分类筛选。 */
    @Nullable private CommunityRequest.Category requestCategory() {
        return kind == CommunityRequest.Kind.SKIN ? category : null;
    }

    private void load(boolean fresh) {
        View view = getView();
        if (view == null) return;
        // Only paging defers to a request already in flight. A new tab or a new search must go out even mid-load, or switching tabs while the first page is arriving does nothing at all; the reply that was already on its way is discarded below by the same check.
        if (loading && !fresh) return;
        loading = true;
        if (fresh) {
            hasMore = false;
            section = kind;
            adapter.set(List.of());
            state("正在载入…", false);
        }
        CommunityRequest.Kind segment = kind;
        CommunityRequest.Kind requested = section;
        int offset = adapter.count(requested);
        String term = search;
        CommunityRequest.Category filter = requestCategory();
        HostTask.run(this,
            context -> new CommunityCatalog(context).list(requested, term, offset, filter),
            page -> {
                // The tab, the search or the category may have moved on while this page was in flight. A stale answer must not write over what the user is now looking at, and must not clear the flag belonging to the request that replaced it.
                if (segment != kind || requested != section || !term.equals(search)
                        || filter != requestCategory()) {
                    return;
                }
                loading = false;
                boolean replies = requested == CommunityRequest.Kind.REPLY;
                if (page == null || page.failed()) {
                    // 回复模板只是短语分段的第二个小节：它读不到时短语包照常显示，只是不再往下翻。
                    if (replies && adapter.hasItems()) {
                        hasMore = false;
                        return;
                    }
                    state(page == null ? CommunityRequest.message(null, 0) : page.failure(), true);
                    return;
                }
                if (replies && offset == 0 && !page.items().isEmpty()) adapter.appendHeader(REPLY_SECTION);
                adapter.append(page.items());
                hasMore = page.hasMore();
                if (!hasMore && segment == CommunityRequest.Kind.PHRASE
                        && requested == CommunityRequest.Kind.PHRASE) {
                    // 短语包翻完了，接着翻同一个搜索词下的回复模板。
                    section = CommunityRequest.Kind.REPLY;
                    hasMore = true;
                    load(false);
                    return;
                }
                state(adapter.hasItems() ? "" : emptyMessage(), false);
            });
    }

    private String emptyMessage() {
        CommunityRequest.Category filter = requestCategory();
        String title = kind == CommunityRequest.Kind.PHRASE ? "短语或回复模板" : kind.title();
        String what = filter == null ? title : "「" + filter.label() + "」分类的" + title;
        return search.isEmpty()
            ? "社区里还没有公开的" + what + "。"
            : "没有找到匹配「" + search + "」的" + what + "。";
    }

    private void state(String message, boolean retryable) {
        View view = getView();
        if (view == null) return;
        TextView state = view.findViewById(R.id.community_state);
        state.setText(message);
        Ui.setVisibilityForText(state, message);
        ViewPolicy.setVisible(view.findViewById(R.id.community_retry), retryable);
    }

    /**
     * 读出本机已经拿到的作品，让卡片上的按钮显示「使用」或「已添加」：皮肤库里的设计、装成命名词库的社区词库、装进常用语的短语包。
     */
    private void refreshTaken() {
        HostTask.run(this, CommunityFragment::taken, values -> {
            if (values != null && adapter != null) adapter.setActions(values);
        });
    }

    private static Map<String, CommunityAdapter.Action> taken(Context context) {
        List<CustomSkinLibrary.Item> skins = List.of();
        String directory = HostStore.directory(context);
        if (!directory.isEmpty()) {
            try {
                skins = CustomSkinLibrary.read(Paths.get(directory));
            } catch (java.io.IOException | RuntimeException error) {
                android.util.Log.i("MSIMECommunity", "Custom skin library unreadable", error);
            }
        }
        DictionaryCollectionsStore.Result<DictionaryCollectionsStore.View> collections =
            DictionaryCollectionsStore.load(context);
        List<DictionaryCollectionsStore.Collection> collectionItems = collections.ok()
            ? collections.value().collections() : List.of();
        CommonPhrasesStore.Result phrases = CommonPhrasesStore.load(context);
        List<CommonPhrasesStore.Pack> packs = phrases.ok()
            ? phrases.document().packs() : List.of();
        Map<String, CommunityAdapter.Action> values = new HashMap<>(
            skins.size() + collectionItems.size() + packs.size());
        for (CustomSkinLibrary.Item skin : skins) {
            values.put(CommunityAdapter.key(skin.id()), CommunityAdapter.Action.DONE);
        }
        for (DictionaryCollectionsStore.Collection collection : collectionItems) {
            if ("community".equals(collection.sourceType()) && collection.resourceId() != null
                    && !collection.resourceId().isEmpty()) {
                values.put(CommunityAdapter.key(collection.resourceId()), CommunityAdapter.Action.DONE);
            }
        }
        for (CommonPhrasesStore.Pack pack : packs) {
            values.put(CommunityAdapter.key(pack.id()), CommunityAdapter.Action.DONE);
        }
        return values;
    }

    private void open(CommunityCatalog.Item item) {
        CommunityAdapter.Action action = adapter.action(item);
        String label = CommunityAdapter.label(item, action);
        boolean live = !label.isEmpty() && action != CommunityAdapter.Action.BUSY
            && !(item.kind() != CommunityRequest.Kind.SKIN && action == CommunityAdapter.Action.DONE);
        CommunitySkinSheet.show(requireContext(), item, nineKey, label,
            live ? () -> act(item) : null,
            () -> report(item),
            item.owned() && item.category() != null ? next -> changeCategory(item, next) : null);
    }

    /** The pill (or the sheet's button) was pressed. */
    private void act(CommunityCatalog.Item item) {
        CommunityAdapter.Action action = adapter.action(item);
        if (action == CommunityAdapter.Action.BUSY) return;
        switch (item.kind()) {
            case SKIN -> {
                if (action == CommunityAdapter.Action.DONE) use(item); else get(item);
            }
            case DICTIONARY, PHRASE -> {
                if (action == CommunityAdapter.Action.AVAILABLE) add(item);
            }
            case REPLY -> { }
        }
    }

    /** 获取：存进皮肤库（键盘皮肤面板读的同一份），成功后在服务端记一次下载。 */
    private void get(CommunityCatalog.Item item) {
        adapter.setAction(item.id(), CommunityAdapter.Action.BUSY);
        HostTask.run(this, context -> {
            String directory = HostStore.directory(context);
            if (directory.isEmpty()) return "键盘还没有完成首次准备，请先打开一次键盘。";
            String failure = new CommunityCatalog(context).install(Paths.get(directory), item);
            if (failure.isEmpty()) SyncSignals.markDirty(context, SyncSwitch.SKINS);
            return failure;
        }, failure -> {
            if (getView() == null) return;
            boolean saved = failure != null && failure.isEmpty();
            adapter.setAction(item.id(), saved ? CommunityAdapter.Action.DONE : CommunityAdapter.Action.AVAILABLE);
            MsToast.show(requireContext(), saved ? "已获取「" + item.name() + "」，点「使用」换上"
                : failure == null ? "保存失败，请稍后重试。" : failure);
            if (saved) HostTask.runNetwork(this, context -> new CommunityCatalog(context).recordDownload(item), ignored -> { });
        });
    }

    /** 使用：与设置里的皮肤页选中「我的设计」写法相同，换上这款设计并记一次换皮肤。 */
    private void use(CommunityCatalog.Item item) {
        JSONObject design = item.payload();
        if (design == null) return;
        adapter.setAction(item.id(), CommunityAdapter.Action.BUSY);
        HostTask.run(this, context -> {
            JSONObject saved = KeyboardSheets.write(context, preferences ->
                KeyboardSheets.applyDesign(preferences, design));
            if (saved == null) return Boolean.FALSE;
            KeyboardSheets.applyLocalFeedback(context, design);
            KeyboardSheets.recordSkin(context, item.id());
            return Boolean.TRUE;
        }, applied -> {
            if (getView() == null) return;
            adapter.setAction(item.id(), CommunityAdapter.Action.DONE);
            MsToast.show(requireContext(), Boolean.TRUE.equals(applied)
                ? "已换上「" + item.name() + "」" : "切换失败，保留当前皮肤");
        });
    }

    /** 添加：社区词库装成命名词库，短语包装进无编码常用语；两者都会标记各自的云同步分类。 */
    private void add(CommunityCatalog.Item item) {
        if (item.raw() == null) return;
        adapter.setAction(item.id(), CommunityAdapter.Action.BUSY);
        boolean dictionary = item.kind() == CommunityRequest.Kind.DICTIONARY;
        HostTask.run(this, context -> {
            if (dictionary) {
                DictionaryCollectionsStore.Result<DictionaryCollectionsStore.View> result =
                    DictionaryCollectionsStore.installCommunity(context, item.raw());
                return result.ok() ? "" : result.failure();
            }
            CommonPhrasesStore.Result result = CommonPhrasesStore.installPack(context, item.raw());
            return result.ok() ? "" : result.failure();
        }, failure -> {
            if (getView() == null) return;
            boolean added = failure != null && failure.isEmpty();
            adapter.setAction(item.id(), added ? CommunityAdapter.Action.DONE : CommunityAdapter.Action.AVAILABLE);
            MsToast.show(requireContext(), added
                ? "已添加「" + item.name() + "」" + (dictionary ? "，在词库里可以停用" : "到常用语")
                : failure == null || failure.isEmpty() ? "添加失败，请重试" : failure);
        });
    }

    /** 举报：one of the fixed reasons and an optional detail, sent with this device's account (the anonymous one counts). */
    private void report(CommunityCatalog.Item item) {
        Context context = requireContext();
        int padding = Ui.dp(context, 20);
        LinearLayout form = Ui.column(context);
        ViewPolicy.setPadding(form, padding, Ui.dp(context, 8), padding, 0);
        RadioGroup reasons = new RadioGroup(context);
        for (String reason : CommunityRequest.REPORT_REASONS) {
            RadioButton choice = new RadioButton(context);
            choice.setId(View.generateViewId());
            choice.setText(reason);
            choice.setTag(reason);
            reasons.addView(choice);
        }
        form.addView(reasons);
        TextInputLayout detailField = new TextInputLayout(context);
        detailField.setHint("补充说明（可选）");
        detailField.setCounterEnabled(true);
        detailField.setCounterMaxLength(CommunityRequest.MAX_REPORT_DETAIL);
        TextInputEditText detail = new TextInputEditText(detailField.getContext());
        ViewPolicy.setMaxLines(detail, 4);
        detailField.addView(detail);
        form.addView(detailField);

        AlertDialog dialog = new MaterialAlertDialogBuilder(context)
            .setTitle("举报「" + item.name() + "」")
            .setView(form)
            .setNegativeButton("取消", null)
            .setPositiveButton("提交", null)
            .create();
        dialog.setOnShowListener(ignored -> {
            View submit = dialog.getButton(AlertDialog.BUTTON_POSITIVE);
            ViewPolicy.setEnabled(submit, false);
            reasons.setOnCheckedChangeListener((group, checked) -> ViewPolicy.setEnabled(submit, checked != -1));
            ViewPolicy.bindClick(submit, () -> {
                View checked = reasons.findViewById(reasons.getCheckedRadioButtonId());
                String reason = checked == null ? "" : String.valueOf(checked.getTag());
                String text = TextPolicy.trimmed(detail.getText() == null ? null : detail.getText().toString());
                if (!CommunityRequest.validReport(reason, text)) {
                    detailField.setError("最多 " + CommunityRequest.MAX_REPORT_DETAIL + " 字");
                    return;
                }
                dialog.dismiss();
                HostTask.runNetwork(this, worker -> new CommunityCatalog(worker).report(item, reason, text),
                    failure -> {
                        if (getView() == null) return;
                        MsToast.show(requireContext(), failure == null || failure.isEmpty()
                            ? "已收到举报，管理员会尽快处理。" : failure);
                    });
            });
        });
        dialog.show();
    }

    private void changeCategory(CommunityCatalog.Item item, CommunityRequest.Category next) {
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).setCategory(item, next),
            update -> {
                if (getView() == null) return;
                if (update == null || update.failed()) {
                    MsToast.show(requireContext(), update == null ? CommunityRequest.message(null, 0)
                        : update.failure());
                    return;
                }
                // 正在按分类筛选时，改到别的分类的那一款就不属于这一页了。
                CommunityRequest.Category filter = requestCategory();
                adapter.replace(update.item(), filter == null || filter == next);
                if (!adapter.hasItems()) state(emptyMessage(), false);
                MsToast.show(requireContext(), "已改为「" + next.label() + "」分类。");
            });
    }
}
