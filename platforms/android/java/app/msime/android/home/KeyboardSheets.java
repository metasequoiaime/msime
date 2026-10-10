package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DrawablePolicy;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.R;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 皮肤、AI 设计皮肤、键盘、AI 润色与回复、输入、表达这几页共用的偏好读写和几种自定义行。
 *
 * <p>这些页面原来是键盘页六个方块背后的底部面板，内容已经搬进各自的详情页（{@link SkinsPage}、{@link KeyboardOptionsPage}、{@link AiSettingsPage}、{@link TypingPage}、{@link ExpressionPage}），这里只留下它们都要用的部分。每次写入都在工作线程上重新读一遍共享偏好再改，按 revision 做 compare-and-swap：键盘是另一个进程，页面打开以后它可能已经改过设置。
 */
final class KeyboardSheets {
    private KeyboardSheets() {}

    /** 对偏好文档 `preferences` 对象的一次修改。 */
    interface Edit {
        void apply(JSONObject preferences) throws JSONException;
    }

    /** 在工作线程上读出当前偏好；还没完成首次设置或读不到时为 null。 */
    @Nullable static JSONObject preferences(Context context) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        return snapshot == null ? null : snapshot.optJSONObject("preferences");
    }

    /** 读一遍、改一处、写回；返回写好的快照，被拒绝或失败时为 null。要在工作线程上调用。 */
    @Nullable static JSONObject write(Context context, Edit edit) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        if (snapshot == null) return null;
        try {
            edit.apply(snapshot.getJSONObject("preferences"));
        } catch (JSONException error) {
            return null;
        }
        return HostStore.savePreferences(context, snapshot);
    }

    /**
     * 在后台写一处修改；失败时提示并调用 `failed`（页面一般重新读一遍，把控件拨回磁盘上的值）。
     *
     * @param saved 写成功后在主线程上调用，可为 null
     */
    static void save(Fragment fragment, Edit edit, @Nullable Runnable saved, Runnable failed) {
        HostTask.run(fragment, context -> write(context, edit), result -> {
            if (result == null) {
                MsToast.show(fragment.requireContext(), "保存失败，请重试");
                failed.run();
            } else if (saved != null) {
                saved.run();
            }
        });
    }

    /** `parent` 里名为 `key` 的子对象，没有就新建一个放进去。 */
    static JSONObject child(JSONObject parent, String key) throws JSONException {
        JSONObject value = parent.optJSONObject(key);
        if (value == null) {
            value = new JSONObject();
            parent.put(key, value);
        }
        return value;
    }

    /**
     * 选中一个自定义设计：与键盘自己的皮肤面板（`MSIMEInputService.saveKeyboardSkin`）写法相同——原来显示的主题记进 `custom_theme.base` 并清掉浅色、深色两个槽位的皮肤包（`candidate_skin` 和 `candidate_skin_dark`），设计写进 `custom_theme.keyboard`，`global_theme` 改成 `custom`；另外按 P23 写设计带的按键音包（静音不改音包）。按键动画和本地按键音开关不在共享偏好里，由 {@link #applyLocalFeedback} 另写。
     */
    static void applyDesign(JSONObject preferences, JSONObject design) throws JSONException {
        String current = InputViewValuePolicy.textOr(preferences, "global_theme", "system");
        JSONObject customTheme = child(preferences, "custom_theme");
        if (!"custom".equals(current)) {
            customTheme.put("base", current);
            customTheme.remove("candidate_skin");
            customTheme.remove("candidate_skin_dark");
        }
        customTheme.put("keyboard", new JSONObject(design.toString()));
        preferences.put("global_theme", "custom");
        CustomKeyboardSkin skin = CustomKeyboardSkin.from(design);
        if (!CustomKeyboardSkin.SILENT_SOUND_PACK.equals(skin.soundPack()))
            child(child(preferences, "plugins"), "key_sound").put("pack", skin.soundPack());
    }

    /**
     * 在后台写一项 Android 本地设置（{@link AndroidLocalSettings}）；失败时提示并调用 `failed`。
     *
     * @param value 新值；null 表示删掉这一项、回到默认值
     * @param saved 写成功后在主线程上调用，可为 null
     */
    static void saveLocal(Fragment fragment, String key, @Nullable Object value, @Nullable Runnable saved,
            Runnable failed) {
        HostTask.run(fragment, context -> writeLocal(context, key, value) ? Boolean.TRUE : null, result -> {
            if (result == null) {
                MsToast.show(fragment.requireContext(), "保存失败，请重试");
                failed.run();
            } else if (saved != null) {
                saved.run();
            }
        });
    }

    /** 写一项 Android 本地设置，参与同步的项同时标记设置有改动。要在工作线程上调用；写不进时为 false。 */
    static boolean writeLocal(Context context, String key, @Nullable Object value) {
        try {
            AndroidLocalSettings.put(context, key, value);
        } catch (java.io.IOException | IllegalArgumentException error) {
            android.util.Log.w("MSIMESettings", "Android local setting was not saved", error);
            return false;
        }
        if (AndroidLocalSettings.spec(key).synced) SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
        return true;
    }

    /** 选中自定义设计后，按 P23 写它带的按键反馈里不在共享偏好的部分：按键音包拨本地按键音开关，按键动画写本地设置。要在工作线程上调用。 */
    static void applyLocalFeedback(Context context, JSONObject design) {
        CustomKeyboardSkin skin = CustomKeyboardSkin.from(design);
        applyLocalSound(context, skin.soundPack());
        Object animation = AndroidLocalSettings.spec(AndroidLocalSettings.KEY_ANIMATION).accept(skin.pressAnimation());
        if (animation != null) writeLocal(context, AndroidLocalSettings.KEY_ANIMATION, animation);
    }

    /** 按设计带的按键音包拨 Android 本地的按键音开关：静音关掉，其他打开。要在工作线程上调用。 */
    static void applyLocalSound(Context context, String soundPack) {
        KeyboardFeedbackStore.Settings settings = KeyboardFeedbackStore.load(context);
        boolean sound = !CustomKeyboardSkin.SILENT_SOUND_PACK.equals(soundPack);
        if (settings.soundEnabled() == sound) return;
        try {
            KeyboardFeedbackStore.save(context, new KeyboardFeedbackStore.Settings(sound,
                settings.hapticsEnabled(), settings.hapticStrength()));
            SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
        } catch (java.io.IOException error) {
            android.util.Log.w("MSIMESettings", "Key sound switch was not saved", error);
        }
    }

    /** 统计里记下用过这款皮肤（成就用），并告诉云同步皮肤有改动；记不下不影响换皮肤。要在工作线程上调用。 */
    static void recordSkin(Context context, String id) {
        try {
            HostStore.statisticsAction(context, new JSONObject().put("operation", "record_skin").put("id", id));
        } catch (JSONException error) {
            android.util.Log.w("MSIMESettings", "Skin use was not recorded", error);
        }
        SyncSignals.markDirty(context, SyncSwitch.SKINS);
    }

    /** 季节 id 的中文名，用在「水杉四季 · 秋杉」这样的卡名上；还没解析过季节时按基础主题秋杉。 */
    static String seasonTitle(@Nullable String season) {
        if (season == null) return "秋杉";
        return switch (season) {
            case "spring" -> "春芽";
            case "summer" -> "夏荫";
            case "winter" -> "冬雪";
            default -> "秋杉";
        };
    }

    // ---- 行 ----

    /** 设计里语言卡那种行：左边一个字的徽标，标题和副标题，行尾是值和 ›。 */
    static View badgeNavRow(Context context, String badge, String title, @Nullable String subtitle,
            @Nullable String value, Runnable action) {
        return badgeNavRow(context, badge, title, subtitle, value, ThemeColorPolicy.subText(context), action);
    }

    /** 徽标导航行的尾部值可使用强调色，供已启用状态等页面复用。 */
    static View badgeNavRow(Context context, String badge, String title, @Nullable String subtitle,
            @Nullable String value, int valueColor, Runnable action) {
        LinearLayout row = baseRow(context);
        row.addView(badge(context, badge));
        row.addView(texts(context, title, subtitle, ThemeColorPolicy.text(context)),
            Ui.weightWrap(1f));
        if (value != null && !value.isEmpty()) {
            TextView state = Ui.trailingValue(context, value, Ui.TEXT_ROW_TITLE, valueColor);
            LinearLayout.LayoutParams params = Ui.rowGapParams(context);
            row.addView(state, params);
        }
        ImageView chevron = Ui.chevron(context);
        LinearLayout.LayoutParams chevronParams = Ui.squareParams(context, Ui.CHEVRON_SIZE);
        chevronParams.setMarginStart(Ui.dp(context, 6));
        row.addView(chevron, chevronParams);
        row.setContentDescription(title
            + (subtitle == null || subtitle.isEmpty() ? "" : "，" + subtitle)
            + (value == null || value.isEmpty() ? "" : "，" + value));
        Ui.makeClickable(row, context, action);
        return row;
    }

    /** 行首一个强调色符号、强调色标题的动作行（「＋ 添加语言」）。 */
    static View actionRow(Context context, String glyph, String title, Runnable action) {
        return actionRow(context, glyph, title, action, 32, Ui.ROW_GAP, 0);
    }

    /**
     * 行首一个强调色符号、强调色标题的动作行；尺寸参数用于复用略有不同密度的词库操作行。
     */
    static View actionRow(Context context, String glyph, String title, Runnable action,
            int iconSize, int iconMarginEnd, int labelMarginStart) {
        return accentActionRow(context, glyph, title, action, iconSize, iconMarginEnd, labelMarginStart);
    }

    /** 强调色动作行的通用构造器，允许页面选择是否显示图标。 */
    static View accentActionRow(Context context, @Nullable String glyph, String title, Runnable action,
            int iconSize, int iconMarginEnd, int labelMarginStart) {
        LinearLayout row = baseRow(context);
        Ui.setMinimumHeightDp(row, context, Ui.COMPACT_ROW_MIN_HEIGHT);
        if (glyph != null) {
            TextView icon = Ui.styledLabel(context, glyph, 22, 400, ThemeColorPolicy.accent(context));
            ViewPolicy.setCentered(icon);
            Ui.hideFromAccessibility(icon);
            LinearLayout.LayoutParams iconParams = Ui.squareParams(context, iconSize);
            iconParams.setMarginEnd(Ui.dp(context, iconMarginEnd));
            row.addView(icon, iconParams);
        }
        TextView label = Ui.styledLabel(context, title, Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.accent(context));
        LinearLayout.LayoutParams labelParams = Ui.weightWrap(1f);
        labelParams.setMarginStart(Ui.dp(context, labelMarginStart));
        row.addView(label, labelParams);
        Ui.makeClickable(row, context, action);
        row.setAccessibilityDelegate(buttonDelegate(title));
        return row;
    }

    /**
     * 徽标、标题、副标题和行尾一个 tonal 胶囊按钮的行（发现短语、添加语言）。
     *
     * @param action 为 null 时按钮是「已添加」这类终态：次要文字色、没有底色、不响应
     */
    static View pillRow(Context context, String badge, String title, @Nullable String subtitle,
            String label, @Nullable Runnable action) {
        LinearLayout row = baseRow(context);
        row.addView(badge(context, badge));
        row.addView(texts(context, title, subtitle, ThemeColorPolicy.text(context)),
            Ui.weightWrap(1f));
        boolean enabled = action != null;
        TextView button;
        if (enabled) {
            button = Ui.pillButton(context, label, Ui.TEXT_BUTTON_SMALL, 500,
                ThemeColorPolicy.accentSoft(context), ThemeColorPolicy.accent(context), Ui.BUTTON_PADDING_H, Ui.BUTTON_PADDING_V,
                Ui.COMPACT_BUTTON_MIN_HEIGHT, 0, action);
        } else {
            button = Ui.styledLabel(context, label, Ui.TEXT_BUTTON_SMALL, 500, ThemeColorPolicy.subText(context));
            ViewPolicy.setCentered(button);
            ViewPolicy.setSingleLine(button);
            Ui.setButtonPadding(button, context);
            Ui.setTextMinHeightDp(button, context, Ui.COMPACT_BUTTON_MIN_HEIGHT);
            ViewPolicy.setEnabled(button, false);
        }
        button.setAccessibilityDelegate(buttonDelegate(label + "，" + title));
        LinearLayout.LayoutParams params = Ui.rowGapParams(context);
        row.addView(button, params);
        return row;
    }

    /** 页面底部的大按钮：主按钮是实心强调色，次按钮是卡片底、正文色；52dp 高、r16。 */
    static TextView bigButton(Context context, String label, boolean primary, Runnable action) {
        TextView button = Ui.textButton(context, label, Ui.TEXT_ROW_TITLE, 600,
            primary ? ThemeColorPolicy.onAccent(context) : ThemeColorPolicy.text(context),
            Ui.rippleOn(context, primary ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.card(context), Ui.dp(context, 16)),
            Ui.ACTION_BUTTON_MIN_HEIGHT, action);
        Ui.setHorizontalPaddingDp(button, context, 16);
        button.setAccessibilityDelegate(buttonDelegate(label));
        return button;
    }

    static LinearLayout baseRow(Context context) {
        LinearLayout row = Ui.row(context);
        ViewPolicy.setCenteredVertically(row);
        Ui.setRowMinimumHeight(row, context);
        Ui.setRowPadding(row, context);
        return row;
    }

    /** 32dp 的圆角方块徽标，供词库和语言行共用。 */
    static TextView badge(Context context, String text) {
        TextView badge = Ui.styledLabel(context, text, 15, 600, ThemeColorPolicy.accent(context));
        ViewPolicy.setCentered(badge);
        ViewPolicy.setBackground(badge, DrawablePolicy.rounded(ThemeColorPolicy.accentSoft(context), Ui.dp(context, 8)));
            Ui.hideFromAccessibility(badge);
        LinearLayout.LayoutParams params = Ui.squareParams(context, 32);
        params.setMarginEnd(Ui.dp(context, Ui.ROW_GAP));
        badge.setLayoutParams(params);
        return badge;
    }

    static LinearLayout texts(Context context, String title, @Nullable String subtitle, int titleColor) {
        LinearLayout texts = Ui.column(context);
        TextView heading = Ui.styledLabel(context, title, Ui.TEXT_ROW_TITLE, 400, titleColor);
        ViewPolicy.setSingleLine(heading);
        texts.addView(heading);
        if (subtitle != null && !subtitle.isEmpty()) {
            TextView detail = Ui.styledLabel(context, subtitle, Ui.TEXT_ROW_SUBTITLE, 400,
                ThemeColorPolicy.subText(context));
            ViewPolicy.setSingleLine(detail);
            texts.addView(detail);
        }
        return texts;
    }

    static View.AccessibilityDelegate buttonDelegate(CharSequence description) {
        return new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Button.class.getName());
                info.setContentDescription(description);
            }
        };
    }

    /** 「我的」页面共用的图标导航行，支持副标题、尾部值和可选点击行为。 */
    static LinearLayout iconNavRow(Context context, @DrawableRes int icon, CharSequence title,
            @Nullable CharSequence subtitle, @Nullable CharSequence value, @Nullable Runnable action) {
        LinearLayout row = Ui.row(context);
        ViewPolicy.setCenteredVertically(row);
        Ui.setMinimumHeightDp(row, context,
            subtitle == null ? Ui.COMPACT_ROW_MIN_HEIGHT : Ui.ROW_MIN_HEIGHT);
        Ui.setRowPadding(row, context);

        ImageView glyph = Ui.decorativeIcon(context, icon, ThemeColorPolicy.subText(context));
        LinearLayout.LayoutParams glyphParams = Ui.squareParams(context, 22);
        glyphParams.setMarginEnd(Ui.dp(context, 18));
        row.addView(glyph, glyphParams);

        LinearLayout texts = Ui.column(context);
        TextView heading = Ui.styledLabel(context, title, Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        texts.addView(heading);
        if (subtitle != null) {
            TextView detail = Ui.styledLabel(context, subtitle, 12, 400, ThemeColorPolicy.subText(context));
            texts.addView(detail);
        }
        row.addView(texts, Ui.weightWrap(1f));

        if (value != null && value.length() > 0) {
            TextView trailing = Ui.trailingValue(context, value, Ui.TEXT_ROW_SUBTITLE, ThemeColorPolicy.subText(context));
            LinearLayout.LayoutParams valueParams = Ui.rowGapParams(context);
            row.addView(trailing, valueParams);
        }
        if (action != null) {
            ImageView chevron = Ui.chevron(context);
            LinearLayout.LayoutParams chevronParams = Ui.squareParams(context, Ui.CHEVRON_SIZE);
            chevronParams.setMarginStart(Ui.dp(context, 6));
            row.addView(chevron, chevronParams);
            Ui.makeClickable(row, context, action);
        }
        return row;
    }

    /** 构造详情卡片行尾的 tonal 胶囊按钮；调用方只需绑定业务点击行为。 */
    static TextView tonalButton(Context context, CharSequence label, CharSequence description, int weight) {
        TextView button = Ui.pillButton(context, label, Ui.TEXT_BUTTON_SMALL, weight,
            ThemeColorPolicy.accentSoft(context), ThemeColorPolicy.accent(context), Ui.BUTTON_PADDING_H, Ui.BUTTON_PADDING_V,
            Ui.COMPACT_BUTTON_MIN_HEIGHT, 0);
        button.setAccessibilityDelegate(buttonDelegate(description));
        ViewPolicy.setPoliteLiveRegion(button);
        return button;
    }

    /** 构造并绑定详情卡片行尾的 tonal 胶囊按钮。 */
    static TextView tonalButton(Context context, CharSequence label, CharSequence description, int weight,
            Runnable action) {
        TextView button = tonalButton(context, label, description, weight);
        ViewPolicy.bindClick(button, action);
        return button;
    }
}
