package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import com.google.android.material.bottomsheet.BottomSheetDialog;
import com.google.android.material.bottomsheet.BottomSheetDragHandleView;
import java.util.List;
import org.json.JSONObject;

/**
 * 「我的 → 应用主题」的选择面板：顶部是颜色模式分段（跟随系统 / 浅色 / 深色，共享偏好 `theme`），下面是水杉四季（自动）与四个固定季节（Android 本地设置的 `general.app_theme`，见 {@link AndroidLocalSettings#APP_THEME}）。
 *
 * <p>外观与 {@link OptionSheet} 一致（M3 modal bottom sheet、居中小标题、56dp 强调色选项、当前项加粗打 ✓、末尾「取消」），只是多了顶部那一行分段，所以自己搭而不是往 OptionSheet 里塞。季节规则在 Rust：写入应用主题后交给 {@link AppThemeController#follow} 重新解析并更新缓存，缓存的季节变了才 `recreate()`；颜色模式写入后交给 {@link AppMode#follow}，由 AppCompat 重建打开着的页面。
 */
final class AppThemeSheet {
    /** 应用主题的取值与显示名，顺序即面板顺序；与 Rust `AppTheme` 的序列化值一致。 */
    static final String[][] THEMES = {
        {"siji", "水杉四季（自动）"}, {"chunya", "春芽"}, {"xiayin", "夏荫"}, {"qiushan", "秋杉"}, {"dongxue", "冬雪"},
    };
    private static final String[][] MODES = {
        {AppMode.SYSTEM, "跟随系统"}, {AppMode.LIGHT, "浅色"}, {AppMode.DARK, "深色"},
    };

    private AppThemeSheet() {}

    /** 当前应用主题在「我的」行尾的写法：四季时带上这一季，例如「四季 · 秋杉」。 */
    static String summary(Context context, @Nullable JSONObject preferences) {
        String theme = theme(context);
        if (!"siji".equals(theme)) return label(theme);
        String season = seasonName(AppThemeController.cachedSeason(context));
        return "四季 · " + season;
    }

    /**
     * 打开面板。
     *
     * @param preferences 页面最近读到的偏好，用来标出当前项；可空
     * @param refresh 保存成功但不需要重建页面时调用，让调用方重读偏好刷新行尾的值
     */
    static void show(Fragment host, @Nullable JSONObject preferences, Runnable refresh) {
        Context context = host.requireContext();
        BottomSheetDialog dialog = new BottomSheetDialog(context);
        LinearLayout root = new LinearLayout(context);
        root.setOrientation(LinearLayout.VERTICAL);
        root.addView(new BottomSheetDragHandleView(context), Ui.matchWidth());

        LinearLayout header = new LinearLayout(context);
        header.setOrientation(LinearLayout.VERTICAL);
        header.setGravity(Gravity.CENTER_HORIZONTAL);
        Ui.setSheetHeaderPadding(header, context);
        TextView heading = new TextView(context);
        heading.setText("应用主题");
        heading.setGravity(Gravity.CENTER);
        Ui.style(heading, Ui.TEXT_SHEET_HEADER, 600, Ui.subText(context));
        heading.setAccessibilityHeading(true);
        header.addView(heading);
        TextView note = new TextView(context);
        note.setText("四季会随季节自动更换配色");
        note.setGravity(Gravity.CENTER);
        Ui.style(note, Ui.TEXT_SHEET_HEADER, 400, Ui.subText(context));
        LinearLayout.LayoutParams noteParams = Ui.wrap();
        noteParams.topMargin = Ui.dp(context, 2);
        header.addView(note, noteParams);

        // 颜色模式：键盘和本应用的浅色 / 深色，与应用主题的季节无关。
        String mode = preferences == null ? AppMode.SYSTEM : AppMode.of(preferences);
        SegmentedControl modes = new SegmentedControl(context);
        modes.setContentDescription("颜色模式");
        int selectedMode = 0;
        for (int index = 0; index < MODES.length; index++) if (MODES[index][0].equals(mode)) selectedMode = index;
        modes.setOptions(List.of(MODES[0][1], MODES[1][1], MODES[2][1]), selectedMode);
        modes.setOnSelect(index -> {
            dialog.dismiss();
            save(host, "theme", MODES[index][0], refresh);
        });
        LinearLayout.LayoutParams modeParams = Ui.wrap();
        modeParams.topMargin = Ui.dp(context, 12);
        header.addView(modes, modeParams);
        root.addView(header);

        String current = theme(context);
        for (int index = 0; index < THEMES.length; index++) {
            if (index > 0) root.addView(rule(context));
            String id = THEMES[index][0];
            root.addView(option(context, THEMES[index][1], id.equals(current), Ui.accent(context), () -> {
                dialog.dismiss();
                if (!id.equals(current)) saveAppTheme(host, id, refresh);
            }));
        }

        View band = new View(context);
        band.setBackgroundColor(Ui.page(context));
        root.addView(band, Ui.matchWidthHeight(context, 8));
        root.addView(option(context, "取消", false, Ui.accent(context), dialog::cancel));
        dialog.setContentView(root);
        dialog.show();
    }

    /** 写共享偏好里的颜色模式 `theme`，交给 AppMode 重建打开着的页面。 */
    private static void save(Fragment host, String key, String value, Runnable refresh) {
        HostTask.run(host, context -> {
            JSONObject saved = HostStore.putPreference(context, key, value);
            if (saved == null) return null;
            return new Object[] {saved.optJSONObject("preferences")};
        }, result -> {
            if (result == null) {
                MsToast.show(host.requireContext(), "没有保存，请重试");
                return;
            }
            JSONObject preferences = result[0] instanceof JSONObject saved ? saved : null;
            AppMode.follow(host.requireContext(), preferences);
            refresh.run();
        });
    }

    /** 写本地设置里的应用主题，重新解析季节；季节变了就重建页面。 */
    private static void saveAppTheme(Fragment host, String id, Runnable refresh) {
        HostTask.run(host, context -> {
            try {
                AndroidLocalSettings.put(context, AndroidLocalSettings.APP_THEME, id);
            } catch (java.io.IOException | IllegalArgumentException error) {
                return null;
            }
            SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
            JSONObject loaded = HostStore.loadPreferences(context);
            JSONObject preferences = loaded == null ? null : loaded.optJSONObject("preferences");
            return new Boolean[] {AppThemeController.follow(context, preferences)};
        }, result -> {
            if (result == null) {
                MsToast.show(host.requireContext(), "没有保存，请重试");
                return;
            }
            if (Boolean.TRUE.equals(result[0])) {
                host.requireActivity().recreate();
            } else {
                refresh.run();
            }
        });
    }

    private static String theme(Context context) {
        return AndroidLocalSettings.load(context).choice(AndroidLocalSettings.APP_THEME);
    }

    private static String label(String theme) {
        for (String[] entry : THEMES) if (entry[0].equals(theme)) return entry[1];
        return THEMES[0][1];
    }

    /** Rust 解析出的季节（spring / summer / autumn / winter）的中文名；没有缓存时是基础主题的秋杉。 */
    static String seasonName(@Nullable String season) {
        if (season == null) return "秋杉";
        switch (season) {
            case "spring": return "春芽";
            case "summer": return "夏荫";
            case "winter": return "冬雪";
            default: return "秋杉";
        }
    }

    private static View option(Context context, CharSequence label, boolean selected, int color, Runnable action) {
        return SheetOptionView.create(context, label, selected, false, color, selected, action);
    }

    private static View rule(Context context) {
        View rule = Ui.hairlineView(context);
        rule.setLayoutParams(Ui.matchWidthHeightPx(Ui.hairlinePx(context)));
        return rule;
    }

}
