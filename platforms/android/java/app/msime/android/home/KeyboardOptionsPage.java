package app.msime.android.home;

import android.content.Context;
import android.graphics.Color;
import android.os.Bundle;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppEdition;
import app.msime.android.KeyboardFeedbackPreferences;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardSkin;
import app.msime.android.SchemePreferences;
import org.json.JSONObject;

/**
 * 键盘页：布局（中文键盘 26 / 9 键、键盘高度、按键间距、行间距）、按键反馈（按键音、按键振动、按键弹出预览、按键动画）、手势（下滑输入符号、空格滑动移动光标、长按空格语音）、键盘工具栏（预览、显示方式、各按钮）和子页「AI 润色与回复」。
 *
 * <p>键盘与本页读同一批存储：按键间距、行间距和表情/剪贴板/皮肤三个工具栏按钮在共享偏好里（`touch_key_spacing_tenths`、`touch_row_spacing_tenths`、`touch_toolbar.*`）；键盘高度、按键弹出预览、按键动画、三个手势、常用语/输入方式两个工具栏按钮和「显示方式：隐藏」（整行不显示，候选条照常显示）只有 Android 用，在 {@link AndroidLocalSettings} 里。键盘高度按设计以 75–130 % 显示，存的是 dp（{@link KeyboardGeometry#heightPercentToAdjustment}），本地没写过时沿用共享偏好里旧的 `touch_keyboard_height_adjustment`。按键音和按键振动是 Android 一直以来的本地开关（`KeyboardFeedbackStore`），键盘的功能面板改的也是它们。
 */
public final class KeyboardOptionsPage extends DetailPage {
    private static final String[] ANIMATIONS = {"bounce", "ripple", "glow", "lift", "none"};
    private static final String[] ANIMATION_LABELS = {"弹起", "涟漪", "发光", "浮起", "无"};
    /** 设计里的五个工具栏按钮：存储键与行名。共享偏好的写 `touch_toolbar` 的成员名，本地设置的写完整键名。 */
    private static final String[][] TOOLBAR_BUTTONS = {
        {"emoji", "表情"}, {AndroidLocalSettings.TOOLBAR_PHRASE, "常用语"}, {"clipboard", "剪贴板"}, {"skin", "皮肤"},
        {AndroidLocalSettings.TOOLBAR_SCHEME, "输入方式"},
    };

    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local,
                         KeyboardFeedbackStore.Settings feedback, KeyboardSkin skin) {}

    @Nullable private LinearLayout column;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void reload() {
        boolean dark = AppMode.dark(requireContext());
        HostTask.run(this, context -> {
            JSONObject preferences = KeyboardSheets.preferences(context);
            if (preferences == null) return null;
            return new State(preferences, AndroidLocalSettings.load(context), KeyboardFeedbackStore.load(context),
                HostStore.keyboardSkin(preferences, dark, HostStore.seed(context)));
        }, this::render);
    }

    private void render(@Nullable State state) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject preferences = state.preferences();
        AndroidLocalSettings.Snapshot settings = state.local();
        AppEdition edition = AppEdition.current();
        KeyboardScheme current = KeyboardScheme.fromPreferences(
            preferences.optString("scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);

        GroupCard layout = GroupCard.add(target, "布局");
        boolean nineKey = "nine_key".equals(current.touchKeyboardLayout());
        boolean quanpinOffered = KeyboardScheme.QUANPIN.offeredBy(edition) && KeyboardScheme.QUANPIN_NINE_KEY.offeredBy(edition);
        layout.nav("中文键盘", quanpinOffered ? null : "本版本只有一种键盘", nineKey ? "9 键" : "26 键",
            quanpinOffered ? () -> pickLayout(nineKey) : null);
        int height = KeyboardGeometry.heightAdjustmentToPercent(
            settings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                ? settings.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                : KeyboardGeometry.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE));
        layout.slider("键盘高度", KeyboardGeometry.MIN_HEIGHT_PERCENT, KeyboardGeometry.MAX_HEIGHT_PERCENT, 1, height,
            KeyboardGeometry::displayPercent,
            percent -> saveLocal(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT,
                KeyboardGeometry.heightPercentToAdjustment(percent)));
        layout.slider("按键间距", KeyboardGeometry.MIN_KEY_SPACING_TENTHS, KeyboardGeometry.MAX_KEY_SPACING_TENTHS, 1,
            KeyboardGeometry.keySpacing(KeyboardGeometry.strictInt(preferences, "touch_key_spacing_tenths", -1)),
            KeyboardGeometry::display, value -> savePreference("touch_key_spacing_tenths", value));
        layout.slider("行间距", KeyboardGeometry.MIN_ROW_SPACING_TENTHS, KeyboardGeometry.MAX_ROW_SPACING_TENTHS, 1,
            KeyboardGeometry.rowSpacing(KeyboardGeometry.strictInt(preferences, "touch_row_spacing_tenths", -1)),
            KeyboardGeometry::display, value -> savePreference("touch_row_spacing_tenths", value));

        GroupCard feedback = GroupCard.add(target, "按键反馈");
        KeyboardFeedbackStore.Settings local = state.feedback();
        feedback.toggle("按键音", null, local.soundEnabled(), checked -> saveFeedback(checked, null));
        feedback.toggle("按键振动", null, local.hapticsEnabled(), checked -> saveFeedback(null, checked));
        feedback.toggle("按键弹出预览", "按下时在键上方显示放大的字符",
            settings.bool(AndroidLocalSettings.KEY_POPUP), checked -> saveLocal(AndroidLocalSettings.KEY_POPUP, checked));
        String animation = settings.choice(AndroidLocalSettings.KEY_ANIMATION);
        GroupCard.Row[] animationRow = new GroupCard.Row[1];
        animationRow[0] = feedback.nav("按键动画", null, animationLabel(animation),
            () -> pickAnimation(animation, animationRow[0]));

        GroupCard gestures = GroupCard.add(target, "手势");
        gestures.toggle("下滑输入符号", "在字母键上向下滑动，输入角标符号",
            settings.bool(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS),
            checked -> saveLocal(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS, checked));
        gestures.toggle("空格键滑动移动光标", null, settings.bool(AndroidLocalSettings.SPACE_CURSOR),
            checked -> saveLocal(AndroidLocalSettings.SPACE_CURSOR, checked));
        gestures.toggle("长按空格语音输入", null, settings.bool(AndroidLocalSettings.SPACE_VOICE),
            checked -> saveLocal(AndroidLocalSettings.SPACE_VOICE, checked));

        JSONObject toolbar = preferences.optJSONObject("touch_toolbar");
        if (toolbar == null) toolbar = new JSONObject();
        boolean hidden = settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN);
        GroupCard bar = GroupCard.add(target, "键盘工具栏");
        bar.addView(toolbarPreview(requireContext(), toolbar, settings, state.skin()));
        bar.nav("显示方式", null, hidden ? "隐藏" : "输入时显示", () -> pickToolbarMode(hidden));
        for (String[] button : TOOLBAR_BUTTONS) {
            String member = button[0];
            GroupCard.Row row = bar.toggle(button[1], null, toolbarButton(toolbar, settings, member),
                checked -> saveToolbar(member, checked));
            row.setEnabled(!hidden);
        }

        GroupCard more = GroupCard.add(target, "更多");
        more.nav("AI 润色与回复", "端点、模型、凭据和提示词", null,
            () -> SettingsNavigator.open(requireContext(), PageId.AI_SETTINGS, null));
    }

    /** 一个工具栏按钮开着没有：本地设置的看本地设置，其余看 `touch_toolbar`（缺键为开）。 */
    private static boolean toolbarButton(JSONObject toolbar, AndroidLocalSettings.Snapshot settings, String member) {
        return AndroidLocalSettings.specs().containsKey(member) ? settings.bool(member) : toolbar.optBoolean(member, true);
    }

    /** 工具栏预览：用当前皮肤的底色和图标色，按开关列出会出现的按钮；隐藏时说明只剩候选条。 */
    private static View toolbarPreview(Context context, JSONObject toolbar, AndroidLocalSettings.Snapshot settings,
                                       KeyboardSkin skin) {
        LinearLayout strip = new LinearLayout(context);
        strip.setOrientation(LinearLayout.HORIZONTAL);
        strip.setGravity(Gravity.CENTER_VERTICAL);
        int pad = Ui.dp(context, 12);
        strip.setPadding(pad, pad, pad, pad);
        LinearLayout plate = new LinearLayout(context);
        plate.setOrientation(LinearLayout.HORIZONTAL);
        plate.setGravity(Gravity.CENTER_VERTICAL);
        plate.setPadding(Ui.dp(context, 10), 0, Ui.dp(context, 10), 0);
        plate.setBackground(Ui.rounded(parse(skin.background(), Ui.page(context)), Ui.dp(context, 12)));
        plate.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        int icon = parse(skin.toolbarIcon(), Ui.subText(context));
        if (settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN)) {
            TextView note = new TextView(context);
            note.setText("工具栏已隐藏，只显示候选条");
            Ui.style(note, 13, 400, icon);
            plate.addView(note);
        } else {
            addChip(context, plate, "水杉", parse(skin.accentText(), Ui.accent(context)));
            for (String[] button : TOOLBAR_BUTTONS) {
                if (toolbarButton(toolbar, settings, button[0])) addChip(context, plate, button[1], icon);
            }
        }
        strip.addView(plate, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Ui.dp(context, 44)));
        strip.setContentDescription("工具栏预览");
        return strip;
    }

    private static void addChip(Context context, LinearLayout plate, String label, int colour) {
        TextView chip = new TextView(context);
        chip.setText(label);
        chip.setSingleLine(true);
        Ui.style(chip, 12, 500, colour);
        LinearLayout.LayoutParams params = KeyboardSheets.wrap();
        params.setMarginEnd(Ui.dp(context, 12));
        plate.addView(chip, params);
    }

    private static int parse(String colour, int fallback) {
        if (colour == null || colour.isEmpty()) return fallback;
        try {
            return Color.parseColor(colour);
        } catch (IllegalArgumentException error) {
            return fallback;
        }
    }

    private void pickLayout(boolean nineKey) {
        OptionSheet sheet = new OptionSheet(requireContext(), "中文键盘", null);
        sheet.option("26 键", !nineKey, () -> applyScheme(KeyboardScheme.QUANPIN));
        sheet.option("9 键", nineKey, () -> applyScheme(KeyboardScheme.QUANPIN_NINE_KEY));
        sheet.show();
    }

    /** 26 / 9 键就是全拼的两个触屏方案；与输入页、引导页一样经 {@link SchemePreferences#withScheme} 一起写那几个键。 */
    private void applyScheme(KeyboardScheme scheme) {
        HostTask.run(this, context -> {
            JSONObject snapshot = HostStore.loadPreferences(context);
            JSONObject pending = SchemePreferences.withScheme(snapshot, scheme, null);
            return pending == null ? null : HostStore.savePreferences(context, pending);
        }, saved -> {
            if (saved == null) MsToast.show(requireContext(), "保存失败，键盘保留当前布局");
            reload();
        });
    }

    private void pickAnimation(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "按键动画", null);
        for (int index = 0; index < ANIMATIONS.length; index++) {
            String value = ANIMATIONS[index];
            String label = ANIMATION_LABELS[index];
            sheet.option(label, value.equals(selected), () -> {
                row.setValue(label);
                saveLocal(AndroidLocalSettings.KEY_ANIMATION, value);
            });
        }
        sheet.show();
    }

    private void pickToolbarMode(boolean hidden) {
        OptionSheet sheet = new OptionSheet(requireContext(), "显示方式", null);
        sheet.option("输入时显示", !hidden, () -> saveToolbar(AndroidLocalSettings.TOOLBAR_HIDDEN, false));
        sheet.option("隐藏", hidden, () -> saveToolbar(AndroidLocalSettings.TOOLBAR_HIDDEN, true));
        sheet.show();
    }

    private static String animationLabel(String value) {
        for (int index = 0; index < ANIMATIONS.length; index++) {
            if (ANIMATIONS[index].equals(value)) return ANIMATION_LABELS[index];
        }
        return ANIMATION_LABELS[ANIMATION_LABELS.length - 1];
    }

    private void savePreference(String key, Object value) {
        KeyboardSheets.save(this, preferences -> preferences.put(key, value), null, this::reload);
    }

    private void saveLocal(String key, Object value) {
        KeyboardSheets.saveLocal(this, key, value, null, this::reload);
    }

    /** 改一个工具栏开关（`touch_toolbar` 的成员或本地设置），写完重画预览（显示方式还会改变按钮行的可用状态）。 */
    private void saveToolbar(String member, boolean value) {
        if (AndroidLocalSettings.specs().containsKey(member)) {
            KeyboardSheets.saveLocal(this, member, value, this::reload, this::reload);
            return;
        }
        KeyboardSheets.save(this, preferences -> KeyboardSheets.child(preferences, "touch_toolbar").put(member, value),
            this::reload, this::reload);
    }

    /** 本地的按键音 / 按键振动；传 null 的那一项保持不变。 */
    private void saveFeedback(@Nullable Boolean sound, @Nullable Boolean haptics) {
        HostTask.run(this, context -> {
            KeyboardFeedbackStore.Settings current = KeyboardFeedbackStore.load(context);
            KeyboardFeedbackPreferences.HapticStrength strength = current.hapticStrength();
            try {
                KeyboardFeedbackStore.save(context, new KeyboardFeedbackStore.Settings(
                    sound == null ? current.soundEnabled() : sound,
                    haptics == null ? current.hapticsEnabled() : haptics, strength));
                return Boolean.TRUE;
            } catch (java.io.IOException error) {
                return null;
            }
        }, saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            }
        });
    }
}
