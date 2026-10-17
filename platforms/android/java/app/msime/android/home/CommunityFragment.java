package app.msime.android.home;

import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;

import android.content.Context;
import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.EditorInfo;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.BackendAccount;
import androidx.recyclerview.widget.GridLayoutManager;
import androidx.recyclerview.widget.LinearLayoutManager;
import androidx.recyclerview.widget.RecyclerView;
import app.msime.android.CommonPhrasesStore;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DictionaryCollectionsStore;
import app.msime.android.R;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.ViewPolicy;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.chip.Chip;
import com.google.android.material.chip.ChipGroup;
import com.google.android.material.textfield.TextInputEditText;
import java.nio.file.Paths;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONObject;

/**
 * 一类社区作品的完整列表：皮肤和词库嵌在底部「社区」tab（{@link CommunityTabPage}）的「皮肤 / 词库」两个分类里，皮肤还嵌在皮肤页的社区分类里；短语或 AI 回复模板从对应功能页末尾的「浏览更多…」打开，压在那一页上面，返回回到那一页。
 *
 * <p>独立页面经 {@link PageId#COMMUNITY} 和 {@link SettingsNavigator} 打开，嵌入的分类经子 Fragment 承载；类型放在参数 {@link #ARG_KIND} 里（{@link CommunityRequest.Kind#id()}），缺省或不认识时是皮肤。列表里有搜索、皮肤的分类筛选、详情面板、举报，作者还能在详情里改自己皮肤的分类。
 *
 * <p>发布和评分要登录的账号，这个宿主只带键盘的匿名身份，一个永远回答「请先登录」的发布按钮不如诚实地没有。拿走作品是打开这一页的目的，每张卡片都能做到：皮肤的「获取」存进自定义皮肤库（并记一次下载），之后同一个按钮变成「使用」并把它换到键盘上；词库的「添加」装成命名词库，短语包的「添加」装进常用语；回复模板手机上还不能添加，只能看详情。
 */
public final class CommunityFragment extends Fragment {
    /** 参数：要列出的作品类型，取 {@link CommunityRequest.Kind#id()}。 */
    public static final String ARG_KIND = "kind";
    /** 嵌在别的页面的分类里，由父页提供标题和分类导航。键名带连字符，外部深链的参数键（`HostDeepLink.isAllowedArgKey`）伪造不了它。 */
    private static final String ARG_EMBEDDED = "community-embedded";
    private record LoadedPage(CommunityCatalog.Page page, String sessionId, boolean stable) {}

    /** 这一页列出的作品类型。 */
    private CommunityRequest.Kind kind = CommunityRequest.Kind.SKIN;
    private CommunityAdapter adapter;
    private String search = "";
    // 皮肤的分类筛选，null 是「全部」。只在皮肤页生效，词库和短语没有分类。
    @Nullable private CommunityRequest.Category category;
    private boolean loading;
    private boolean hasMore;
    private long requestGeneration;
    // 预览按用户自己的布局画。读不到就按 26 键，那是默认值。
    private boolean nineKey;

    /** 打开某一类作品的完整列表：皮肤和词库切到「社区」tab 并选中那一类，其余类型压在当前页面上面。 */
    public static void browse(Context context, CommunityRequest.Kind kind) {
        Bundle arguments = new Bundle();
        arguments.putString(ARG_KIND, kind.id());
        SettingsNavigator.open(context, PageId.COMMUNITY, arguments);
    }

    /** 完整列表的页面标题。 */
    static String title(CommunityRequest.Kind kind) {
        return switch (kind) {
            case SKIN -> "社区皮肤";
            case DICTIONARY -> "社区词库";
            case PHRASE -> "社区短语";
            case REPLY -> "AI 回复模板";
        };
    }

    /** 一类作品的列表页；类型为 null 时列皮肤。 */
    public static CommunityFragment forKind(@Nullable CommunityRequest.Kind kind) {
        CommunityFragment fragment = new CommunityFragment();
        if (kind != null) {
            Bundle arguments = new Bundle();
            arguments.putString(ARG_KIND, kind.id());
            fragment.setArguments(arguments);
        }
        return fragment;
    }

    /** 皮肤社区分类直接承载完整列表，由父页提供标题、返回按钮和分类 Tab。 */
    static CommunityFragment skinTab() {
        return embedded(CommunityRequest.Kind.SKIN);
    }

    /** 嵌在父页分类里的完整列表，不显示自己的标题和返回按钮。 */
    static CommunityFragment embedded(CommunityRequest.Kind kind) {
        CommunityFragment fragment = forKind(kind);
        fragment.requireArguments().putBoolean(ARG_EMBEDDED, true);
        return fragment;
    }

    /** 参数里的作品类型；缺省或不认识时是皮肤。 */
    static CommunityRequest.Kind kind(@Nullable Bundle arguments) {
        String id = arguments == null ? null : arguments.getString(ARG_KIND);
        for (CommunityRequest.Kind value : CommunityRequest.kinds()) {
            if (value.id().equals(id)) return value;
        }
        return CommunityRequest.Kind.SKIN;
    }

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_community, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        Bundle arguments = getArguments();
        kind = kind(arguments);
        if (state != null) {
            search = state.getString("community-search", search);
            String selected = state.getString("community-category", "");
            for (CommunityRequest.Category choice : CommunityRequest.categories()) {
                if (choice.id().equals(selected)) category = choice;
            }
        }

        // 和详情页一样：左上角返回，标题是这一类作品。返回走 Activity 的返回分发，弹回打开这一页的功能页。
        ((TextView) view.findViewById(R.id.community_title)).setText(title(kind));
        View back = view.findViewById(R.id.community_back);
        back.setContentDescription("返回");
        ViewPolicy.bindClick(back, () -> requireActivity().getOnBackPressedDispatcher().onBackPressed());
        if (arguments != null && arguments.getBoolean(ARG_EMBEDDED)) {
            ViewPolicy.hide((View) back.getParent());
        }

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
                InputViewValuePolicy.textOr(preferences, "touch_keyboard_layout", "twenty_six_key"));
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
        if (arguments != null && arguments.getBoolean(ARG_EMBEDDED)) PageInsetsPolicy.bind(items);
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
        field.setText(search);
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
        // 打开完整的皮肤列表时顺手把键盘皮肤面板用的目录缓存也刷新一遍，不看缓存新旧。
        if (kind == CommunityRequest.Kind.SKIN) CommunitySkinCatalogue.refresh(this, true, ignored -> { });
    }

    @Override public void onSaveInstanceState(@NonNull Bundle state) {
        super.onSaveInstanceState(state);
        state.putString("community-search", search);
        state.putString("community-category", category == null ? "" : category.id());
    }

    /** 切回社区分类时更新已获取标记，不清空搜索条件和列表。 */
    void refreshTakenMarks() { refreshTaken(); }

    /** 离开社区分类时收起搜索键盘，避免它遮挡其他分类。 */
    void closeSearchKeyboard() {
        View view = getView();
        if (view == null) return;
        View field = view.findViewById(R.id.community_search);
        if (!field.hasFocus()) return;
        android.view.inputmethod.InputMethodManager manager =
            requireContext().getSystemService(android.view.inputmethod.InputMethodManager.class);
        if (manager != null) manager.hideSoftInputFromWindow(field.getWindowToken(), 0);
        field.clearFocus();
    }

    private void updateSearchHint() {
        View view = getView();
        if (view == null) return;
        // 胶囊搜索框没有浮动标签，提示直接写在输入框里。
        ((TextInputEditText) view.findViewById(R.id.community_search)).setHint(kind.searchHint());
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
        long request = ++requestGeneration;
        loading = true;
        if (fresh) {
            hasMore = false;
            adapter.set(List.of());
            state("正在载入…", false);
            // 转圈居中，下面这行字也跟着居中；结果出来后 state() 恢复靠左。
            ViewPolicy.setVisible(view.findViewById(R.id.community_loading), true);
            ViewPolicy.setCentered(view.findViewById(R.id.community_state));
        }
        CommunityRequest.Kind requested = kind;
        int offset = adapter.count(requested);
        String term = search;
        CommunityRequest.Category filter = requestCategory();
        HostTask.run(this,
            context -> {
                BackendAccount account = new BackendAccount(context);
                String before = account.sessionId();
                CommunityCatalog.Page page = new CommunityCatalog(context).list(
                    requested, term, offset, filter);
                String after = account.sessionId();
                return new LoadedPage(page, before, java.util.Objects.equals(before, after));
            },
            loaded -> {
                // 同一筛选条件也可能连续请求；旧结果不能填入新列表或清掉新请求的加载状态。
                if (request != requestGeneration || requested != kind || !term.equals(search)
                        || filter != requestCategory()) {
                    return;
                }
                if (loaded == null || !loaded.stable()
                        || !java.util.Objects.equals(loaded.sessionId(),
                            new BackendAccount(requireContext()).sessionId())) {
                    loading = false;
                    state("账号已切换，请重试。", true);
                    return;
                }
                loading = false;
                CommunityCatalog.Page page = loaded.page();
                if (page == null || page.failed()) {
                    state(page == null ? CommunityRequest.message(null, 0) : page.failure(), true);
                    return;
                }
                adapter.append(page.items());
                hasMore = page.hasMore();
                state(adapter.hasItems() ? "" : emptyMessage(), false);
            });
    }

    private String emptyMessage() {
        CommunityRequest.Category filter = requestCategory();
        String title = kind.title();
        String what = filter == null ? title : "「" + filter.label() + "」分类的" + title;
        return search.isEmpty()
            ? "社区里还没有公开的" + what + "。"
            : "没有找到匹配「" + search + "」的" + what + "。";
    }

    /** 提示行和重试键；转圈只在 {@link #load} 发出新的首屏请求时打开，这里之后的每个结果（列表、空列表、失败）都把它关掉。 */
    private void state(String message, boolean retryable) {
        View view = getView();
        if (view == null) return;
        ViewPolicy.setVisible(view.findViewById(R.id.community_loading), false);
        TextView state = view.findViewById(R.id.community_state);
        state.setGravity(android.view.Gravity.START);
        ViewPolicy.setTextAndVisibility(state, message);
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

    /** 举报：对话框与 AI 页的回复模板共用。 */
    private void report(CommunityCatalog.Item item) {
        CommunityReportDialog.show(this, item);
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
