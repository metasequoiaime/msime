package app.msime.android.home;

import android.content.Intent;
import android.content.res.ColorStateList;
import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.annotation.StringRes;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppEdition;
import app.msime.android.FirstRunPreparation;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardSkin;
import app.msime.android.R;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.core.InputViewValuePolicy;
import com.google.android.material.button.MaterialButton;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

/**
 * 设置 tab：键盘有没有设置好、现在是什么样、一个试用入口，以及各组设置的入口。
 *
 * <p>按设计的 Android 设置首页排。状态卡回答决定键盘能不能用的两件事——系统里启用了没有、是不是默认输入法——并把补上缺的那一步的操作放在旁边；卡里还有系统输入法设置的链接和整宽的「试用键盘」。下面的分组按 P11：[皮肤, 键盘] [输入, 表达, 词库] [语音输入, 手写输入] [开发者选项]，每行的副标题是当前值，点开经 {@link SettingsNavigator#open} 压入详情页。搜索胶囊同时找 {@link PageId} 的标题与关键词和本页各行。
 */
public final class KeyboardFragment extends HomeTabFragment {

    @Nullable private JSONObject snapshot;
    private AndroidLocalSettings.Snapshot local = AndroidLocalSettings.defaults();
    /** The current scheme's title for the status line, or null until the preferences are read. */
    @Nullable private String schemeTitle;
    private final android.view.ViewTreeObserver.OnWindowFocusChangeListener focusWatch = focused -> {
        if (focused && isAdded() && !isHidden()) renderStatus();
    };
    private boolean loaded;
    private boolean prepared = true;
    @Nullable private FirstRunPreparation.Listener preparationListener;
    /** 首页的一行和搜索按什么文字匹配它。 */
    private record HomeRow(PageId page, @DrawableRes int icon, String title, HomeNavGroup.Row row) {}
    private final ArrayList<HomeRow> homeRows = new ArrayList<>(9);

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_keyboard, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        View card = view.findViewById(R.id.keyboard_status_card);
        ViewPolicy.setBackground(card, Ui.rounded(Ui.card(requireContext()),
            Ui.dp(requireContext(), Ui.NAV_GROUP_RADIUS)));

        MaterialButton trial = view.findViewById(R.id.keyboard_try);
        // The Apple app opens an editor here rather than the system picker: trying the keyboard
        // means typing with it, and the picker only offers to switch away from it.
        ViewPolicy.bindClick(trial,
            () -> startActivity(new Intent(requireContext(), KeyboardTryoutActivity.class)));

        TextView system = view.findViewById(R.id.keyboard_system_settings);
        system.setText("系统输入法设置 ›");
        ViewPolicy.bindClick(system, this::openInputMethodSettings);

        // The system's input method picker is a dialog over this window, so there is no resume when it closes; the returning focus is the only sign the default may have changed.
        view.getViewTreeObserver().addOnWindowFocusChangeListener(focusWatch);

        SearchPill search = view.findViewById(R.id.keyboard_search);
        search.setHint(getString(R.string.keyboard_search_hint));
        search.setOnQueryChange(query -> applySearch());

        View bar = view.findViewById(R.id.keyboard_bar);
        int threshold = Ui.dp(requireContext(), Ui.COLLAPSE_THRESHOLD);
        NestedScrollView scroll = view.findViewById(R.id.keyboard_scroll);
        scroll.setOnScrollChangeListener(
            (NestedScrollView.OnScrollChangeListener) (scrolled, x, y, oldX, oldY) -> {
                boolean collapsed = y > threshold;
                if (collapsed == (bar.getVisibility() == View.VISIBLE)) return;
                bar.animate().cancel();
                if (collapsed) {
                    ViewPolicy.show(bar);
                    bar.animate().alpha(1f).setDuration(Ui.APP_BAR_FADE_MILLIS).start();
                } else {
                    ViewPolicy.setInvisible(bar);
                    bar.setAlpha(0f);
                }
            });
        // 和 DetailPage 一样：底部留出 tab 栏加系统导航栏的高度，键盘弹出时改留键盘的高度，最后一组和搜索结果才能滚到可见处。
        int base = Ui.dp(requireContext(), Ui.PAGE_PADDING_BOTTOM);
        int tabs = Ui.dp(requireContext(), Ui.TAB_BAR_HEIGHT);
        ViewCompat.setOnApplyWindowInsetsListener(scroll, (target, insets) -> {
            Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars());
            Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());
            int bottom = Ui.bottomContentInset(bars.bottom, tabs, ime.bottom, base);
            Ui.setBottomPadding(target, bottom);
            return insets;
        });
        ViewCompat.requestApplyInsets(scroll);
        buildRows(view);
        render();
        reload();

        // Preparation is silent while it works out and while it is done; it only takes the screen
        // when the keyboard cannot reach the Engine, which is the one case the user has to know.
        TextView preparation = view.findViewById(R.id.keyboard_preparation);
        ViewPolicy.setBackground(preparation, Ui.rounded(Ui.page(requireContext()), Ui.dp(requireContext(), 12)));
        ViewPolicy.bindClick(preparation, () -> FirstRunPreparation.retry(requireContext()));
        preparationListener = status -> {
            if (!isAdded()) return;
            switch (status) {
                case RUNNING -> {
                    preparation.setText(R.string.preparation_running);
                    ViewPolicy.setClickable(preparation, false);
                    ViewPolicy.show(preparation);
                }
                case FAILED -> {
                    String reason = FirstRunPreparation.failure();
                    // 原因直接写在提示里：出问题的多是别人手里的手机，没法让用户连电脑看 logcat。
                    if (reason.isEmpty()) preparation.setText(R.string.preparation_failed);
                    else preparation.setText(getString(R.string.preparation_failed_reason, reason));
                    ViewPolicy.setClickable(preparation, true);
                    ViewPolicy.show(preparation);
                }
                default -> {
                    ViewPolicy.hide(preparation);
                    // The store only becomes readable once preparation finishes, so the rows have
                    // to be asked again; otherwise they keep saying 尚未准备 until the tab is left.
                    reload();
                }
            }
        };
        FirstRunPreparation.observe(preparationListener);
    }

    @Override public void onDestroyView() {
        if (preparationListener != null) FirstRunPreparation.stopObserving(preparationListener);
        preparationListener = null;
        View view = getView();
        if (view != null) view.getViewTreeObserver().removeOnWindowFocusChangeListener(focusWatch);
        homeRows.clear();
        super.onDestroyView();
    }

    // The keyboard's own pickers write the same file, so what this tab shows can go stale while
    // the user is in the keyboard rather than in here.
    // Notices are fetched when the app home comes on screen; the shared store caches the feed for a minute.
    @Override protected void onBecameVisible() {
        reload();
        View view = getView();
        if (view != null) NoticeBanner.load(this, view.findViewById(R.id.keyboard_notices));
    }

    private void reload() {
        HostTask.run(this, context -> {
            boolean ready = HostStore.prepared(context);
            JSONObject loadedSnapshot = ready ? HostStore.loadPreferences(context) : null;
            // 应用主题要读本地设置文件并调两次 Rust 解析，留在工作线程做；主线程只比较缓存的季节。
            if (loadedSnapshot != null) AppThemeController.follow(context, loadedSnapshot.optJSONObject("preferences"));
            return new Object[] {ready, loadedSnapshot, AndroidLocalSettings.load(context)};
        }, result -> {
            if (result != null) {
                prepared = Boolean.TRUE.equals(result[0]);
                if (result[2] instanceof AndroidLocalSettings.Snapshot settings) local = settings;
                if (result[1] instanceof JSONObject value) {
                    snapshot = value;
                    JSONObject preferences = value.optJSONObject("preferences");
                    // The colour mode may have been changed in the keyboard, the shared settings page or by sync since the host last looked; a change recreates this activity in the new mode.
                    AppMode.follow(requireContext(), preferences);
                    // 应用主题同理：换了主题或换了季节，HomeActivity 用新季节重建。
                    if (getActivity() instanceof HomeActivity home) home.recreateIfSeasonChanged();
                }
            }
            loaded = true;
            render();
        });
    }

    /** P11 的四组行；只建一次，值随偏好原地改写。 */
    private void buildRows(View view) {
        LinearLayout rows = view.findViewById(R.id.keyboard_rows);
        rows.removeAllViews();
        homeRows.clear();
        HomeNavGroup appearance = HomeNavGroup.add(rows);
        row(appearance, PageId.SKINS, R.drawable.ic_ms_palette);
        row(appearance, PageId.KEYBOARD_OPTIONS, R.drawable.ic_ms_keyboard);
        HomeNavGroup typing = HomeNavGroup.add(rows);
        row(typing, PageId.TYPING, R.drawable.ic_ms_text_fields);
        row(typing, PageId.EXPRESSION, R.drawable.ic_ms_translate);
        row(typing, PageId.LEXICON, R.drawable.ic_ms_menu_book);
        HomeNavGroup other = HomeNavGroup.add(rows);
        row(other, PageId.VOICE, R.drawable.ic_ms_mic);
        row(other, PageId.HANDWRITING, R.drawable.ic_ms_edit);
        HomeNavGroup developer = HomeNavGroup.add(rows);
        row(developer, PageId.DEVELOPER, R.drawable.ic_ms_build);
    }

    private void row(HomeNavGroup group, PageId page, @DrawableRes int icon) {
        HomeNavGroup.Row row = group.add(icon, page.title(), null, () -> open(page));
        homeRows.add(new HomeRow(page, icon, page.title(), row));
    }

    private void open(PageId page) {
        SettingsNavigator.open(requireContext(), page, null);
    }

    private void render() {
        View view = getView();
        if (view == null) return;
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");

        // Three states, and they are not the same sentence: still reading, never prepared, and
        // prepared but unreadable. A fresh install is the second one, and calling that a failure
        // sends the user looking for a fault instead of at the one step that is missing.
        String pending = !loaded ? "读取中…" : prepared ? "读取失败" : "尚未准备";
        String scheme = pending;
        String skin = pending;
        if (preferences != null) {
            // The page's own night mode stands in for the system's, as the keyboard's does: with `theme` on 跟随系统 the host follows the system, and with it forced the resolver never asks.
            KeyboardSkin resolved = HostStore.keyboardSkin(preferences, AppMode.dark(requireContext()),
                HostStore.seed(requireContext()));
            String layout = InputViewValuePolicy.textOr(preferences, "touch_keyboard_layout", "twenty_six_key");
            AppEdition edition = AppEdition.current();
            KeyboardScheme selected = KeyboardScheme.fromPreferences(
                preferences.optString("scheme", edition.defaultScheme()),
                preferences.optString("shuangpin_profile", "xiaohe"), layout, edition);
            skin = resolved.title();
            scheme = selected.title(InputViewValuePolicy.textOr(preferences, "wubi_profile", KeyboardScheme.WUBI_86));
        }
        schemeTitle = preferences == null ? null : scheme;
        for (HomeRow entry : homeRows) {
            entry.row().setValue(preferences == null ? pending : value(entry.page(), preferences, skin, scheme));
        }
        renderStatus();
        applySearch();
    }

    /** 每行副标题上的当前值。 */
    private String value(PageId page, JSONObject preferences, String skin, String scheme) {
        switch (page) {
            case SKINS: return skin;
            case KEYBOARD_OPTIONS: return keysSummary(preferences, local);
            case TYPING: return scheme;
            case EXPRESSION:
                return preferences.optBoolean("chinese_punctuation", true) ? "中文标点" : "英文标点";
            case LEXICON:
                return preferences.optBoolean("learning", true) ? "记忆新词已开" : "记忆新词已关";
            case VOICE: return voiceLanguage(preferences);
            case HANDWRITING: return handwritingMode(local);
            case DEVELOPER: return developerSummary(local);
            default: return "";
        }
    }

    // ---- search ----

    /**
     * 按搜索胶囊里的文字过滤。
     *
     * <p>有查询时状态卡、通知和分组让开，下面列出两类结果：本页标题或当前值命中的行，以及标题或关键词命中的其他可搜索页面（{@link PageId#matches}），后者的副标题是命中的那个设置项。什么都没找到时明说，而不是留一页空白。
     */
    private void applySearch() {
        View view = getView();
        if (view == null) return;
        String query = TextPolicy.lowercase(
            ((SearchPill) view.findViewById(R.id.keyboard_search)).query());
        boolean active = !query.isEmpty();
        ViewPolicy.setVisible(view.findViewById(R.id.keyboard_notices), !active);
        ViewPolicy.setVisible(view.findViewById(R.id.keyboard_status_card), !active);
        ViewPolicy.setVisible(view.findViewById(R.id.keyboard_rows), !active);
        LinearLayout results = view.findViewById(R.id.keyboard_search_results);
        results.removeAllViews();
        ViewPolicy.setVisible(results, active);
        int shown = 0;
        if (active) {
            HomeNavGroup group = null;
            List<PageId> listed = new ArrayList<>(homeRows.size());
            for (HomeRow entry : homeRows) {
                CharSequence value = ((TextView) entry.row().view().findViewById(R.id.row_value)).getText();
                String text = TextPolicy.lowercase(entry.title() + " " + value);
                if (!text.contains(query) && !entry.page().matches(query)) continue;
                if (group == null) group = HomeNavGroup.add(results);
                PageId page = entry.page();
                group.add(entry.icon(), entry.title(), matchedKeyword(page, query, value), () -> open(page));
                listed.add(page);
                shown++;
            }
            for (PageId page : PageId.values()) {
                if (listed.contains(page) || !page.searchable() || !page.matches(query)) continue;
                if (group == null) group = HomeNavGroup.add(results);
                group.add(R.drawable.ic_search, page.title(), matchedKeyword(page, query, null),
                    () -> open(page));
                shown++;
            }
        }
        ViewPolicy.setVisible(view.findViewById(R.id.keyboard_search_empty), active && shown == 0);
    }

    /** 搜索结果的副标题：命中的关键词；只命中标题时用这一行原来的值。 */
    @Nullable private static CharSequence matchedKeyword(PageId page, String query,
            @Nullable CharSequence fallback) {
        for (String keyword : page.keywords()) {
            if (TextPolicy.lowercase(keyword).contains(query)) return keyword;
        }
        return fallback;
    }

    // ---- status ----

    /**
     * The status card: the two setup checks, each with the one action that completes it.
     *
     * <p>Enabled and default are separate facts. A keyboard can be enabled and never used because another one stays the default, and the design's single 已启用 line only holds when both are true.
     */
    private void renderStatus() {
        View view = getView();
        if (view == null) return;
        boolean enabled = ImeSetup.enabled(requireContext());
        boolean current = enabled && ImeSetup.isDefault(requireContext());
        String state = getString(prepared && current
            ? R.string.settings_status_ready : R.string.settings_status_incomplete);
        ((TextView) view.findViewById(R.id.keyboard_status_line)).setText(
            schemeTitle == null ? state : state + " · " + schemeTitle);
        check(view, R.id.keyboard_check_enabled, R.id.keyboard_check_enabled_mark,
            R.id.keyboard_check_enabled_action, R.string.settings_check_enabled, enabled,
            this::openInputMethodSettings);
        check(view, R.id.keyboard_check_default, R.id.keyboard_check_default_mark,
            R.id.keyboard_check_default_action, R.string.settings_check_default, current,
            () -> ImeSetup.makeDefault(requireContext()));
    }

    private void check(View view, int rowId, int markId, int actionId, @StringRes int label,
            boolean done, Runnable action) {
        TextView mark = view.findViewById(markId);
        Ui.applyStatusMark(mark, requireContext(), done);
        TextView button = view.findViewById(actionId);
        ViewPolicy.setVisible(button, !done);
        ViewPolicy.bindOptionalClick(button, done ? null : action);
        ViewPolicy.setTextColor(button, ColorStateList.valueOf(Ui.accent(requireContext())));
        view.findViewById(rowId).setContentDescription(
            getString(label) + (done ? "，已完成" : "，未完成"));
    }

    private void openInputMethodSettings() {
        ImeSetup.openSettings(requireContext());
    }

    // ---- row values ----

    private static String keysSummary(JSONObject preferences, AndroidLocalSettings.Snapshot local) {
        // 和键盘页一样按百分比显示：存的是 dp 调整量，MIN_VALUE 表示没有设置（即 100 %），超出范围的旧值先夹紧再换算。
        int percent = KeyboardGeometry.heightAdjustmentToPercent(
            local.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                ? local.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                : KeyboardGeometry.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE));
        String layout = "nine_key".equals(InputViewValuePolicy.textOr(preferences, "touch_keyboard_layout", "twenty_six_key"))
            ? "九键" : "全键盘";
        return percent == KeyboardGeometry.DEFAULT_HEIGHT_PERCENT
            ? layout + " · 标准高度"
            : layout + " · 高度 " + KeyboardGeometry.displayPercent(percent);
    }

    private static String voiceLanguage(JSONObject preferences) {
        JSONObject voice = preferences.optJSONObject("voice_input");
        String language = TextPolicy.lowercase(voice == null ? "" : voice.optString("language", ""));
        if (language.isEmpty() || language.startsWith("zh") || language.startsWith("cmn")) return "普通话";
        if (language.startsWith("yue")) return "粤语";
        if (language.startsWith("en")) return "英语";
        if (language.startsWith("ja")) return "日语";
        return language;
    }

    private static String handwritingMode(AndroidLocalSettings.Snapshot local) {
        switch (local.choice(AndroidLocalSettings.HANDWRITING_MODE)) {
            case "single": return "单字";
            case "line": return "行写";
            default: return "叠写";
        }
    }

    private static String developerSummary(AndroidLocalSettings.Snapshot local) {
        switch (local.choice(AndroidLocalSettings.DEVELOPER_LOG_LEVEL)) {
            case "error": return "日志级别 错误";
            case "info": return "日志级别 信息";
            case "debug": return "日志级别 调试";
            default: return "日志级别 警告";
        }
    }
}
