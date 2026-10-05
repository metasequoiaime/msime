package app.msime.android.home;

import android.content.Context;
import android.content.res.ColorStateList;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.R;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
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
     * 选中一个自定义设计：与键盘自己的皮肤面板（`MSIMEInputService.saveKeyboardSkin`）写法相同——原来显示的主题记进 `custom_theme.base` 并清掉 `candidate_skin`，设计写进 `custom_theme.keyboard`，`global_theme` 改成 `custom`；另外按 P23 写设计带的按键音包（静音不改音包）。按键动画和本地按键音开关不在共享偏好里，由 {@link #applyLocalFeedback} 另写。
     */
    static void applyDesign(JSONObject preferences, JSONObject design) throws JSONException {
        String current = preferences.optString("global_theme", "system");
        JSONObject customTheme = child(preferences, "custom_theme");
        if (!"custom".equals(current)) {
            customTheme.put("base", current);
            customTheme.remove("candidate_skin");
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
        LinearLayout row = baseRow(context);
        row.addView(LexiconPage.badge(context, badge));
        row.addView(texts(context, title, subtitle, Ui.text(context)),
            new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        if (value != null && !value.isEmpty()) {
            TextView state = new TextView(context);
            state.setText(value);
            state.setSingleLine(true);
            Ui.style(state, Ui.TEXT_ROW_TITLE, 400, Ui.subText(context));
            LinearLayout.LayoutParams params = wrap();
            params.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
            row.addView(state, params);
        }
        ImageView chevron = new ImageView(context);
        chevron.setImageResource(R.drawable.ms_w1_a2_chevron);
        chevron.setImageTintList(ColorStateList.valueOf(Ui.subText(context)));
        chevron.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        LinearLayout.LayoutParams chevronParams = new LinearLayout.LayoutParams(
            Ui.dp(context, Ui.CHEVRON_SIZE), Ui.dp(context, Ui.CHEVRON_SIZE));
        chevronParams.setMarginStart(Ui.dp(context, 6));
        row.addView(chevron, chevronParams);
        row.setBackground(Ui.ripple(context));
        row.setClickable(true);
        row.setFocusable(true);
        row.setOnClickListener(ignored -> action.run());
        row.setContentDescription(title + (value == null || value.isEmpty() ? "" : "，" + value));
        return row;
    }

    /** 行首一个强调色符号、强调色标题的动作行（「＋ 添加语言」）。 */
    static View actionRow(Context context, String glyph, String title, Runnable action) {
        LinearLayout row = baseRow(context);
        row.setMinimumHeight(Ui.dp(context, 52));
        TextView icon = new TextView(context);
        icon.setText(glyph);
        icon.setGravity(Gravity.CENTER);
        Ui.style(icon, 22, 400, Ui.accent(context));
        icon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        LinearLayout.LayoutParams iconParams = new LinearLayout.LayoutParams(Ui.dp(context, 32), Ui.dp(context, 32));
        iconParams.setMarginEnd(Ui.dp(context, Ui.ROW_GAP));
        row.addView(icon, iconParams);
        TextView label = new TextView(context);
        label.setText(title);
        Ui.style(label, Ui.TEXT_ROW_TITLE, 400, Ui.accent(context));
        row.addView(label, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        row.setBackground(Ui.ripple(context));
        row.setClickable(true);
        row.setFocusable(true);
        row.setOnClickListener(ignored -> action.run());
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
        row.addView(LexiconPage.badge(context, badge));
        row.addView(texts(context, title, subtitle, Ui.text(context)),
            new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        boolean enabled = action != null;
        TextView button = new TextView(context);
        button.setText(label);
        button.setGravity(Gravity.CENTER);
        button.setSingleLine(true);
        Ui.style(button, Ui.TEXT_BUTTON_SMALL, 500, enabled ? Ui.accent(context) : Ui.subText(context));
        if (enabled) button.setBackground(Ui.rippleOn(context, Ui.accentSoft(context), 9999f));
        button.setPadding(Ui.dp(context, 14), Ui.dp(context, 5), Ui.dp(context, 14), Ui.dp(context, 5));
        button.setMinHeight(Ui.dp(context, 32));
        button.setEnabled(enabled);
        button.setClickable(enabled);
        button.setFocusable(enabled);
        if (enabled) button.setOnClickListener(ignored -> action.run());
        button.setAccessibilityDelegate(buttonDelegate(label + "，" + title));
        LinearLayout.LayoutParams params = wrap();
        params.setMarginStart(Ui.dp(context, Ui.ROW_GAP));
        row.addView(button, params);
        return row;
    }

    /** 页面底部的大按钮：主按钮是实心强调色，次按钮是卡片底、正文色；52dp 高、r16。 */
    static TextView bigButton(Context context, String label, boolean primary, Runnable action) {
        TextView button = new TextView(context);
        button.setText(label);
        button.setGravity(Gravity.CENTER);
        button.setSingleLine(true);
        Ui.style(button, Ui.TEXT_ROW_TITLE, 600, primary ? Ui.onAccent(context) : Ui.text(context));
        button.setBackground(Ui.rippleOn(context, primary ? Ui.accent(context) : Ui.card(context),
            Ui.dp(context, 16)));
        button.setMinHeight(Ui.dp(context, 52));
        button.setPadding(Ui.dp(context, 16), 0, Ui.dp(context, 16), 0);
        button.setClickable(true);
        button.setFocusable(true);
        button.setOnClickListener(ignored -> action.run());
        button.setAccessibilityDelegate(buttonDelegate(label));
        return button;
    }

    static LinearLayout baseRow(Context context) {
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        row.setMinimumHeight(Ui.dp(context, Ui.ROW_MIN_HEIGHT));
        row.setPadding(Ui.dp(context, Ui.ROW_PADDING_H), Ui.dp(context, Ui.ROW_PADDING_V),
            Ui.dp(context, Ui.ROW_PADDING_H), Ui.dp(context, Ui.ROW_PADDING_V));
        return row;
    }

    static LinearLayout texts(Context context, String title, @Nullable String subtitle, int titleColor) {
        LinearLayout texts = new LinearLayout(context);
        texts.setOrientation(LinearLayout.VERTICAL);
        TextView heading = new TextView(context);
        heading.setText(title);
        heading.setSingleLine(true);
        Ui.style(heading, Ui.TEXT_ROW_TITLE, 400, titleColor);
        texts.addView(heading);
        if (subtitle != null && !subtitle.isEmpty()) {
            TextView detail = new TextView(context);
            detail.setText(subtitle);
            detail.setSingleLine(true);
            Ui.style(detail, Ui.TEXT_ROW_SUBTITLE, 400, Ui.subText(context));
            texts.addView(detail);
        }
        return texts;
    }

    static LinearLayout.LayoutParams wrap() {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
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
}
