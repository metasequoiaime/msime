package app.msime.android.home;

import android.content.Intent;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.annotation.StringRes;
import androidx.core.content.ContextCompat;
import androidx.core.widget.NestedScrollView;
import android.widget.LinearLayout;
import app.msime.android.FirstRunPreparation;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardSkin;
import app.msime.android.InputFeatureToggle;
import app.msime.android.R;
import com.google.android.material.button.MaterialButton;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * The 设置 tab: whether the keyboard is set up, what it currently is, a way to try it, and the way in to each group of settings.
 *
 * <p>Laid out as the design's Android settings home. The search pill filters the rows by what they and their sheets contain. The status card answers the two questions that decide whether the keyboard works at all -- enabled in the system, chosen as the default -- and puts the action that fixes the missing one next to it. The groups below open the same bottom sheets the old tiles did; the design's pushed detail pages would need a navigation stack this shell does not have.
 */
public final class KeyboardFragment extends HomeTabFragment {

    @Nullable private JSONObject snapshot;
    /** The current scheme's title for the status line, or null until the preferences are read. */
    @Nullable private String schemeTitle;
    private final android.view.ViewTreeObserver.OnWindowFocusChangeListener focusWatch = focused -> {
        if (focused && isAdded() && !isHidden()) renderStatus();
    };
    private boolean loaded;
    private boolean prepared = true;
    @Nullable private FirstRunPreparation.Listener preparationListener;
    /** A settings row and the text the search pill matches it by. */
    private record SearchEntry(View row, String text) {}
    private final List<SearchEntry> searchable = new ArrayList<>();
    /** The spaces between groups; they only make sense around the full list. */
    private final List<View> gaps = new ArrayList<>();

    @Override public View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup parent,
                                       @Nullable Bundle state) {
        return inflater.inflate(R.layout.page_keyboard, parent, false);
    }

    @Override public void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        MaterialButton trial = view.findViewById(R.id.keyboard_try);
        // The Apple app opens an editor here rather than the system picker: trying the keyboard
        // means typing with it, and the picker only offers to switch away from it.
        trial.setOnClickListener(ignored ->
            startActivity(new Intent(requireContext(), KeyboardTryoutActivity.class)));

        // The system's input method picker is a dialog over this window, so there is no resume when it closes; the returning focus is the only sign the default may have changed.
        view.getViewTreeObserver().addOnWindowFocusChangeListener(focusWatch);

        ((TextView) view.findViewById(R.id.keyboard_search)).addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}
            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}
            @Override public void afterTextChanged(Editable text) { applySearch(); }
        });

        View bar = view.findViewById(R.id.keyboard_bar);
        int threshold = ListRows.dp(requireContext(), 28);
        ((NestedScrollView) view.findViewById(R.id.keyboard_scroll)).setOnScrollChangeListener(
            (NestedScrollView.OnScrollChangeListener) (scroll, x, y, oldX, oldY) -> {
                boolean collapsed = y > threshold;
                if (collapsed == (bar.getVisibility() == View.VISIBLE)) return;
                bar.animate().cancel();
                if (collapsed) {
                    bar.setVisibility(View.VISIBLE);
                    bar.animate().alpha(1f).setDuration(150).start();
                } else {
                    bar.setVisibility(View.INVISIBLE);
                    bar.setAlpha(0f);
                }
            });
        render();
        reload();

        // Preparation is silent while it works out and while it is done; it only takes the screen
        // when the keyboard cannot reach the Engine, which is the one case the user has to know.
        TextView preparation = view.findViewById(R.id.keyboard_preparation);
        preparation.setOnClickListener(ignored -> FirstRunPreparation.retry(requireContext()));
        preparationListener = status -> {
            if (!isAdded()) return;
            switch (status) {
                case RUNNING -> {
                    preparation.setText(R.string.preparation_running);
                    preparation.setClickable(false);
                    preparation.setVisibility(View.VISIBLE);
                }
                case FAILED -> {
                    preparation.setText(R.string.preparation_failed);
                    preparation.setClickable(true);
                    preparation.setVisibility(View.VISIBLE);
                }
                default -> {
                    preparation.setVisibility(View.GONE);
                    // The store only becomes readable once preparation finishes, so the tiles have
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
            return new Object[] {ready, ready ? HostStore.loadPreferences(context) : null};
        }, result -> {
            if (result != null) {
                prepared = Boolean.TRUE.equals(result[0]);
                if (result[1] instanceof JSONObject value) {
                    snapshot = value;
                    // The colour mode may have been changed in the keyboard, the shared settings page or by sync since the host last looked; a change recreates this activity in the new mode.
                    AppMode.follow(requireContext(), value.optJSONObject("preferences"));
                }
            }
            loaded = true;
            render();
        });
    }

    private void render() {
        View view = getView();
        if (view == null) return;
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");

        // Three states, and they are not the same sentence: still reading, never prepared, and
        // prepared but unreadable. A fresh install is the second one, and calling that a failure
        // sends the user looking for a fault instead of at the one step that is missing.
        String pending = !loaded ? "读取中…" : prepared ? "读取失败" : "尚未准备";
        String skin = pending;
        String scheme = pending;
        if (preferences != null) {
            // The page's own night mode stands in for the system's, as the keyboard's does: with `theme` on 跟随系统 the host follows the system, and with it forced the resolver never asks.
            KeyboardSkin resolved = HostStore.keyboardSkin(preferences, AppMode.dark(requireContext()));
            String layout = preferences.optString("touch_keyboard_layout", "twenty_six_key");
            KeyboardScheme selected = KeyboardScheme.fromPreferences(
                preferences.optString("scheme", "quanpin"),
                preferences.optString("shuangpin_profile", "xiaohe"), layout);
            String wubiProfile = preferences.optString("wubi_profile", KeyboardScheme.WUBI_86);
            skin = resolved.title();
            scheme = selected.title(wubiProfile);
            // The picture is of this keyboard, not of a keyboard: a fixed nine-key grid in fixed
            // colours under a caption naming the user's own 26-key layout contradicted itself.
            ((KeyboardPreview) view.findViewById(R.id.keyboard_preview)).setKeyboard(
                resolved, "nine_key".equals(layout), selected.glyph() + selected.badge(wubiProfile));
        }
        ((TextView) view.findViewById(R.id.keyboard_summary)).setText(skin + " · " + scheme);
        schemeTitle = preferences == null ? null : scheme;
        renderStatus();

        boolean ready = ImeSetup.enabled(requireContext());
        LinearLayout rows = view.findViewById(R.id.keyboard_rows);
        rows.removeAllViews();
        searchable.clear();
        gaps.clear();
        // The design's mobile groups, keeping only the pages this host actually has: 候选栏, 键盘工具栏, 语音输入 and 手写输入 have no native settings behind them yet, and a row that opens nothing is worse than no row.
        row(rows, R.drawable.ic_feature_skin, "主题", skin, preferences == null ? null
            : () -> KeyboardSheets.showSkins(this, snapshot, this::reload), themeTerms());
        gap(rows);
        row(rows, R.drawable.ic_feature_scheme, "输入", scheme, preferences == null ? null
            : () -> KeyboardSheets.showSchemes(this, snapshot, this::reload), schemeTerms());
        row(rows, R.drawable.ic_feature_dictionary, "词库", dictionarySummary(preferences),
            preferences == null ? null
                : () -> KeyboardSheets.showInputFeatures(this, snapshot, this::reload),
            featureTerms());
        row(rows, R.drawable.ic_feature_dictionary, "个人词库", "查询、编辑和导入用户词条",
            this::openPersonalDictionary, "词典 用户词 自造词");
        row(rows, R.drawable.ic_feature_dictionary, "背单词", "导入词表，按间隔复习",
            this::openVocabularyReview, "单词 词表 复习 英语");
        gap(rows);
        row(rows, R.drawable.ic_feature_keys, "键盘", keysSummary(preferences),
            preferences == null ? null : () -> KeyboardSheets.showKeys(this, snapshot, this::reload),
            "按键 按键间距 行间距 键盘高度 顶部语音入口 语音");
        row(rows, R.drawable.ic_feature_ai, "AI", aiSummary(preferences),
            preferences == null ? null : () -> KeyboardSheets.showAi(this, snapshot, this::reload),
            "AI 润色与回复 启用 AI 入口 端点 URL 模型 凭据 润色提示词");
        gap(rows);
        row(rows, R.drawable.ic_feature_system, "系统输入法设置",
            ready ? "已启用" : "启用与切换", this::openInputMethodSettings, "启用 默认输入法 切换输入法");
        applySearch();
    }

    // ---- search ----

    private void row(LinearLayout rows, int icon, String title, String value,
            @Nullable Runnable action, String terms) {
        View row = ListRows.add(rows, icon, title, value, action);
        searchable.add(new SearchEntry(row,
            (title + " " + value + " " + terms).toLowerCase(Locale.ROOT)));
    }

    private void gap(LinearLayout rows) {
        ListRows.gap(rows);
        gaps.add(rows.getChildAt(rows.getChildCount() - 1));
    }

    /**
     * Filter the rows by the search pill's text.
     *
     * <p>The design draws the pill and says nothing of what it finds, so it finds what is on this page: each row by its name, its current value and the names of what its sheet holds -- the themes, the schemes, the dictionary toggles, the key sliders and the AI fields. While a query is typed the status and keyboard cards step aside so the matches sit under the pill, and an empty result says so rather than showing a blank page.
     */
    private void applySearch() {
        View view = getView();
        if (view == null) return;
        String query = ((TextView) view.findViewById(R.id.keyboard_search)).getText().toString()
            .trim().toLowerCase(Locale.ROOT);
        boolean active = !query.isEmpty();
        int shown = 0;
        for (SearchEntry entry : searchable) {
            boolean match = !active || entry.text().contains(query);
            entry.row().setVisibility(match ? View.VISIBLE : View.GONE);
            if (match) shown++;
        }
        for (View gap : gaps) gap.setVisibility(active ? View.GONE : View.VISIBLE);
        view.findViewById(R.id.keyboard_notices).setVisibility(active ? View.GONE : View.VISIBLE);
        view.findViewById(R.id.keyboard_status_card).setVisibility(active ? View.GONE : View.VISIBLE);
        view.findViewById(R.id.keyboard_card).setVisibility(active ? View.GONE : View.VISIBLE);
        view.findViewById(R.id.keyboard_search_empty)
            .setVisibility(active && shown == 0 ? View.VISIBLE : View.GONE);
    }

    private static String themeTerms() {
        StringBuilder terms = new StringBuilder("皮肤 外观 颜色模式 跟随系统 浅色 深色");
        JSONArray themes = HostStore.themeCatalog();
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry != null) terms.append(' ').append(entry.optString("title", ""));
        }
        return terms.toString();
    }

    private static String schemeTerms() {
        StringBuilder terms = new StringBuilder("输入方案 拼音 键盘布局");
        for (KeyboardScheme candidate : KeyboardScheme.values()) {
            if (candidate != KeyboardScheme.THOUGHTFUL_REPLY) terms.append(' ').append(candidate.title());
        }
        // 「98 五笔」在输入方案里同样可选，搜得到它才找得到这一行。
        terms.append(' ').append(KeyboardScheme.WUBI.title(KeyboardScheme.WUBI_98));
        return terms.toString();
    }

    private static String featureTerms() {
        StringBuilder terms = new StringBuilder("词库与输入");
        for (InputFeatureToggle.Group group : InputFeatureToggle.groups()) {
            terms.append(' ').append(group.title());
            for (InputFeatureToggle feature : InputFeatureToggle.of(group)) {
                terms.append(' ').append(feature.title());
            }
        }
        return terms.toString();
    }

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
        GradientDrawable disc = new GradientDrawable();
        disc.setShape(GradientDrawable.OVAL);
        disc.setColor(ContextCompat.getColor(requireContext(),
            done ? R.color.forest : R.color.attention));
        mark.setBackground(disc);
        mark.setText(done ? "✓" : "!");
        mark.setTextColor(done ? ContextCompat.getColor(requireContext(), R.color.on_accent)
            : 0xFFFFFFFF);
        MaterialButton button = view.findViewById(actionId);
        button.setVisibility(done ? View.GONE : View.VISIBLE);
        button.setOnClickListener(done ? null : ignored -> action.run());
        view.findViewById(rowId).setContentDescription(
            getString(label) + (done ? "，已完成" : "，未完成"));
    }

    private void openInputMethodSettings() {
        ImeSetup.openSettings(requireContext());
    }

    /** Keep the dictionary editor in the shared Tauri UI while exposing it from the native shell. */
    private void openPersonalDictionary() {
        if (!tauriAvailable()) {
            android.widget.Toast.makeText(requireContext(),
                "个人词库需要管理界面合包，请使用 Tauri 合包打开。", android.widget.Toast.LENGTH_LONG)
                .show();
            return;
        }
        Intent intent = new Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "dictionary");
        startActivity(intent);
    }

    /**
     * Keep the review page in the shared UI while exposing it from the native shell.
     *
     * <p>The same handoff the personal dictionary uses, and for the same reason: the page is one
     * shared React screen that every other host already renders, and a second hand-written copy
     * here would be a second set of rules about intervals and counts to keep in step. The rules
     * themselves are not duplicated either way — they live in `client-core::vocabulary` and this
     * host reaches them through {@code NativeClient.vocabularyReview} — but the screen would be.
     */
    private void openVocabularyReview() {
        if (!tauriAvailable()) {
            android.widget.Toast.makeText(requireContext(),
                "背单词需要管理界面合包，请使用 Tauri 合包打开。", android.widget.Toast.LENGTH_LONG)
                .show();
            return;
        }
        Intent intent = new Intent();
        intent.setClassName(requireContext(), "app.msime.android.MainActivity");
        intent.putExtra("msime_settings_page", "vocabulary");
        startActivity(intent);
    }

    /** The standalone native APK deliberately has no WebView; the Tauri bundle does. */
    private boolean tauriAvailable() {
        try {
            Class.forName("app.msime.android.MainActivity");
            return true;
        } catch (ClassNotFoundException error) {
            return false;
        }
    }

    private String keysSummary(@Nullable JSONObject preferences) {
        if (preferences == null) return !loaded ? "读取中…" : prepared ? "读取失败" : "尚未准备";
        int height = preferences.optInt("touch_keyboard_height_adjustment", 0);
        return height == 0 ? "标准高度" : "高度 " + (height > 0 ? "+" : "") + height;
    }

    private String dictionarySummary(@Nullable JSONObject preferences) {
        if (preferences == null) return !loaded ? "读取中…" : prepared ? "读取失败" : "尚未准备";
        return preferences.optBoolean("learning", true) ? "记忆新词已开" : "记忆新词已关";
    }

    private String aiSummary(@Nullable JSONObject preferences) {
        if (preferences == null) return !loaded ? "读取中…" : prepared ? "读取失败" : "尚未准备";
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        if (ai == null || !ai.optBoolean("enabled", false)) return "未启用";
        String model = ai.optString("model", "");
        return model.isEmpty() ? "已启用" : model;
    }
}
