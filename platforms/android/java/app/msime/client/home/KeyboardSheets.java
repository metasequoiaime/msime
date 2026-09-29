package app.msime.client.home;

import android.content.Context;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.SeekBar;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import androidx.fragment.app.Fragment;
import app.msime.client.InputFeatureToggle;
import app.msime.client.KeyboardGeometry;
import app.msime.client.KeyboardScheme;
import app.msime.client.KeyboardSkin;
import app.msime.client.R;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.textfield.TextInputEditText;
import com.google.android.material.textfield.TextInputLayout;
import java.util.List;
import java.util.function.Consumer;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 键盘页那六个方块背后的面板。
 *
 * <p>Every one of them edits the same shared preferences file the input service reads, through the
 * same compare-and-swap the keyboard's own pickers use. Each sheet reads the snapshot when it opens
 * rather than being handed one: the keyboard is a separate process and may have changed a setting
 * since this tab was drawn.
 */
public final class KeyboardSheets {
    private KeyboardSheets() {}

    /** 皮肤：九种内置配色加上用户自己存的那一套。 */
    public static void showSkins(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        String current = preferences.optString("touch_keyboard_skin", "forest");
        SettingsSheet sheet = new SettingsSheet(context, "键盘皮肤",
            "配色、圆角和键帽材质。皮肤只改外观，不改输入方案。");
        TextView status = sheet.addStatus();
        boolean dark = KeyboardSkin.resolveDark(
            preferences.optString("screen_keyboard_theme", "follow"),
            preferences.optString("theme", "system"), false);
        for (KeyboardSkin skin : KeyboardSkin.builtIns(dark)) {
            sheet.add(choice(context, skin.title(), skin.description(), skin.id().equals(current),
                () -> save(fragment, snapshot, "touch_keyboard_skin", skin.id(), status, sheet,
                    changed)));
        }
        sheet.addNote("自定义皮肤在键盘的皮肤面板里编辑；这里只切换。");
        sheet.show();
    }

    /** 输入方案：一次切换要同时写四个键，交给共享的映射算，不在这里拼。 */
    public static void showSchemes(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        KeyboardScheme current = KeyboardScheme.fromPreferences(
            preferences.optString("scheme", "quanpin"),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"));
        SettingsSheet sheet = new SettingsSheet(context, "输入方案",
            "换方案会同时换掉键盘布局。已经学到的词不受影响。");
        TextView status = sheet.addStatus();
        for (KeyboardScheme scheme : KeyboardScheme.values()) {
            if (scheme == KeyboardScheme.THOUGHTFUL_REPLY) continue;
            sheet.add(choice(context, scheme.title(), scheme.glyph() + " · " + scheme.badge(),
                scheme == current,
                () -> applyScheme(fragment, snapshot, scheme, status, sheet, changed)));
        }
        sheet.addNote("高情商回复是键盘上的一个入口，不是一种输入方案，所以不在这个列表里。");
        sheet.show();
    }

    /** 按键：间距、高度和顶部语音入口，与键盘内的调整面板写同一批键。 */
    public static void showKeys(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        SettingsSheet sheet = new SettingsSheet(context, "按键",
            "间距和高度只改变键位外观，不改变输入方案。");
        TextView status = sheet.addStatus();

        slider(context, sheet, "按键间距",
            KeyboardGeometry.MIN_KEY_SPACING_TENTHS, KeyboardGeometry.MAX_KEY_SPACING_TENTHS,
            KeyboardGeometry.keySpacing(preferences.optInt("touch_key_spacing_tenths", -1)),
            KeyboardGeometry::display,
            value -> save(fragment, snapshot, "touch_key_spacing_tenths", value, status, null,
                changed));
        slider(context, sheet, "行间距",
            KeyboardGeometry.MIN_ROW_SPACING_TENTHS, KeyboardGeometry.MAX_ROW_SPACING_TENTHS,
            KeyboardGeometry.rowSpacing(preferences.optInt("touch_row_spacing_tenths", -1)),
            KeyboardGeometry::display,
            value -> save(fragment, snapshot, "touch_row_spacing_tenths", value, status, null,
                changed));
        slider(context, sheet, "键盘高度",
            KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_DP, KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_DP,
            KeyboardGeometry.heightAdjustment(
                preferences.optInt("touch_keyboard_height_adjustment", Integer.MIN_VALUE)),
            KeyboardGeometry::displayHeight,
            value -> save(fragment, snapshot, "touch_keyboard_height_adjustment", value, status,
                null, changed));

        sheet.add(toggle(context, "顶部语音入口", "在候选行上方常驻一个语音按钮",
            preferences.optBoolean("touch_voice_shortcut", false),
            value -> save(fragment, snapshot, "touch_voice_shortcut", value, status, null,
                changed)));
        sheet.addNote("按键音和震动跟着系统，在键盘的「更多」里开关。");
        sheet.show();
    }

    /** 词库与上屏：一组布尔开关，键与默认值都由 InputFeatureToggle 说了算。 */
    public static void showInputFeatures(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        SettingsSheet sheet = new SettingsSheet(context, "词库与输入",
            "候选怎么来、标点怎么上屏。改动立刻对键盘生效。");
        TextView status = sheet.addStatus();
        for (InputFeatureToggle.Group group : InputFeatureToggle.groups()) {
            sheet.addHeading(group.title());
            for (InputFeatureToggle feature : InputFeatureToggle.of(group)) {
                sheet.add(toggle(context, feature.title(), feature.description(),
                    preferences.optBoolean(feature.key(), feature.enabledByDefault()),
                    value -> save(fragment, snapshot, feature.key(), value, status, null,
                        changed)));
            }
        }
        sheet.addNote("模糊音、辅助码和候选外观在键盘自己的设置面板里，那里能一边改一边看。");
        sheet.show();
    }

    /**
     * AI：端点、模型、提示词和凭据。
     *
     * <p>The token is stored under the endpoint's own origin, which is how the keyboard looks it up
     * and why moving the endpoint to another host does not carry the credential with it.
     */
    public static void showAi(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        SettingsSheet sheet = new SettingsSheet(context, "AI 润色与回复",
            "由你自己的服务端提供。凭据只保存在本机，按端点的来源分别存放。");
        TextView status = sheet.addStatus();

        MaterialSwitch enabled = new MaterialSwitch(context);
        enabled.setText("启用 AI 入口");
        enabled.setChecked(ai != null && ai.optBoolean("enabled", false));
        sheet.add(enabled);

        TextInputLayout endpoint = field(context, "端点 URL",
            ai == null ? "" : ai.optString("endpoint", ""), InputType.TYPE_TEXT_VARIATION_URI);
        TextInputLayout model = field(context, "模型",
            ai == null ? "" : ai.optString("model", ""), InputType.TYPE_CLASS_TEXT);
        String savedOrigin = ai == null ? "" : originOf(ai.optString("endpoint", ""));
        JSONObject tokens = ai == null ? null : ai.optJSONObject("tokens");
        TextInputLayout token = field(context, "凭据",
            tokens == null || savedOrigin.isEmpty() ? "" : tokens.optString(savedOrigin, ""),
            InputType.TYPE_TEXT_VARIATION_PASSWORD);
        TextInputLayout prompt = field(context, "润色提示词",
            ai == null ? "" : ai.optString("prompt", ""), InputType.TYPE_CLASS_TEXT);
        for (TextInputLayout entry : List.of(endpoint, model, token, prompt)) sheet.add(entry);

        MaterialButton save = new MaterialButton(context);
        save.setText("保存");
        save.setOnClickListener(ignored -> {
            JSONObject next = buildAi(ai, enabled.isChecked(), text(endpoint), text(model),
                text(prompt), text(token));
            if (next == null) {
                status.setText("端点必须是一个 https 地址");
                return;
            }
            save(fragment, snapshot, "ai_assistant", next, status, null, changed);
        });
        sheet.add(save);
        sheet.addNote("端点必须是 HTTPS。留空凭据表示该来源不需要凭据，不会清掉其他来源已存的凭据。");
        sheet.show();
    }

    @Nullable private static JSONObject buildAi(@Nullable JSONObject previous, boolean enabled,
            String endpoint, String model, String prompt, String token) {
        if (enabled && !TextPolicy.validAuthority(endpoint, "https://", 2048)) return null;
        try {
            JSONObject next = previous == null ? new JSONObject()
                : new JSONObject(previous.toString());
            next.put("enabled", enabled);
            next.put("endpoint", endpoint);
            next.put("model", model);
            next.put("prompt", prompt);
            String origin = originOf(endpoint);
            if (!origin.isEmpty()) {
                JSONObject tokens = next.optJSONObject("tokens");
                if (tokens == null) tokens = new JSONObject();
                // Only this endpoint's own origin is touched; another host's credential is not
                // this screen's to discard.
                if (token.isEmpty()) tokens.remove(origin); else tokens.put(origin, token);
                next.put("tokens", tokens);
            }
            return next;
        } catch (JSONException error) {
            return null;
        }
    }

    private static String originOf(String endpoint) {
        try {
            return app.msime.client.AiPolishConfiguration.credentialOrigin(endpoint);
        } catch (RuntimeException error) {
            return "";
        }
    }

    private static void applyScheme(Fragment fragment, JSONObject snapshot, KeyboardScheme scheme,
            TextView status, SettingsSheet sheet, Runnable changed) {
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        KeyboardScheme.PreferenceMapping mapping = scheme.mapping(
            preferences.optString("last_chinese_scheme", "quanpin"),
            preferences.optString("shuangpin_profile", "xiaohe"));
        JSONObject pending;
        try {
            pending = new JSONObject(snapshot.toString());
            JSONObject values = pending.getJSONObject("preferences");
            values.put("scheme", mapping.scheme());
            values.put("last_chinese_scheme", mapping.lastChineseScheme());
            values.put("shuangpin_profile", mapping.shuangpinProfile());
            values.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
        } catch (JSONException error) {
            status.setText("切换失败，保留当前方案");
            return;
        }
        status.setText("正在保存…");
        HostTask.run(fragment, context -> HostStore.savePreferences(context, pending), saved -> {
            if (saved == null) {
                status.setText("保存失败，键盘保留当前方案");
                return;
            }
            sheet.dismiss();
            changed.run();
        });
    }

    private static void save(Fragment fragment, JSONObject snapshot, String key, Object value,
            TextView status, @Nullable SettingsSheet sheet, Runnable changed) {
        status.setText("正在保存…");
        HostTask.run(fragment, context -> HostStore.putPreference(context, key, value), saved -> {
            if (saved == null) {
                status.setText("保存失败，键盘保留当前设置");
                return;
            }
            // The snapshot this sheet opened with is now a revision behind; refresh it in place so
            // a second edit in the same sheet is not rejected by the compare-and-swap.
            try {
                snapshot.put("revision", saved.optLong("revision"));
                snapshot.put("preferences", saved.optJSONObject("preferences"));
            } catch (JSONException ignored) {
                // The next edit reloads instead; the write itself already succeeded.
            }
            status.setText("已保存");
            if (sheet != null) sheet.dismiss();
            changed.run();
        });
    }

    @Nullable private static JSONObject preferences(JSONObject snapshot) {
        return snapshot == null ? null : snapshot.optJSONObject("preferences");
    }

    /** The theme's own press indication, resolved once so rows built in code can wear it. */
    private static int rippleBackground(Context context) {
        android.util.TypedValue value = new android.util.TypedValue();
        return context.getTheme().resolveAttribute(
            androidx.appcompat.R.attr.selectableItemBackground, value, true) ? value.resourceId : 0;
    }

    private static String text(TextInputLayout field) {
        return field.getEditText() == null ? ""
            : field.getEditText().getText().toString().trim();
    }

    private static TextInputLayout field(Context context, String hint, String value, int inputType) {
        TextInputLayout layout = new TextInputLayout(context);
        layout.setHint(hint);
        TextInputEditText input = new TextInputEditText(context);
        input.setSingleLine(true);
        input.setInputType(InputType.TYPE_CLASS_TEXT | inputType);
        input.setText(value);
        layout.addView(input);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.topMargin = Math.round(10 * context.getResources().getDisplayMetrics().density);
        layout.setLayoutParams(params);
        return layout;
    }

    private static View toggle(Context context, String title, String description, boolean checked,
            Consumer<Boolean> onChange) {
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.VERTICAL);
        int padding = Math.round(10 * context.getResources().getDisplayMetrics().density);
        row.setPadding(0, padding, 0, padding);
        MaterialSwitch value = new MaterialSwitch(context);
        value.setText(title);
        value.setTextSize(16);
        value.setTextColor(ContextCompat.getColor(context, R.color.ink));
        value.setChecked(checked);
        value.setOnCheckedChangeListener((button, state) -> onChange.accept(state));
        row.addView(value);
        TextView note = new TextView(context);
        note.setText(description);
        note.setTextSize(12);
        note.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
        row.addView(note);
        return row;
    }

    private static View choice(Context context, String title, String description, boolean selected,
            Runnable onPick) {
        LinearLayout row = new LinearLayout(context);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        float density = context.getResources().getDisplayMetrics().density;
        int padding = Math.round(12 * density);
        int inset = Math.round(8 * density);
        row.setPadding(inset, padding, inset, padding);
        row.setClickable(true);
        row.setFocusable(true);
        // A clickable row with no background gives no sign it was pressed, which in a list of nine
        // skins reads as the tap having missed.
        row.setBackgroundResource(rippleBackground(context));
        row.setOnClickListener(ignored -> onPick.run());

        LinearLayout labels = new LinearLayout(context);
        labels.setOrientation(LinearLayout.VERTICAL);
        TextView name = new TextView(context);
        name.setText(title);
        name.setTextSize(16);
        name.setTextColor(ContextCompat.getColor(context, R.color.ink));
        labels.addView(name);
        TextView note = new TextView(context);
        note.setText(description);
        note.setTextSize(12);
        note.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
        labels.addView(note);
        row.addView(labels, new LinearLayout.LayoutParams(0,
            ViewGroup.LayoutParams.WRAP_CONTENT, 1));

        TextView tick = new TextView(context);
        tick.setText(selected ? "✓" : "");
        tick.setTextSize(18);
        tick.setTextColor(ContextCompat.getColor(context, R.color.forest));
        row.addView(tick);
        return row;
    }

    private static void slider(Context context, SettingsSheet sheet, String title, int minimum,
            int maximum, int current, java.util.function.IntFunction<String> format,
            Consumer<Integer> onCommit) {
        LinearLayout header = new LinearLayout(context);
        header.setOrientation(LinearLayout.HORIZONTAL);
        TextView name = new TextView(context);
        name.setText(title);
        name.setTextSize(16);
        name.setTextColor(ContextCompat.getColor(context, R.color.ink));
        header.addView(name, new LinearLayout.LayoutParams(0,
            ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        TextView value = new TextView(context);
        value.setText(format.apply(current));
        value.setTextSize(14);
        value.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
        header.addView(value);
        LinearLayout.LayoutParams headerParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        headerParams.topMargin = Math.round(
            14 * context.getResources().getDisplayMetrics().density);
        sheet.content().addView(header, headerParams);

        SeekBar slider = new SeekBar(context);
        slider.setMax(maximum - minimum);
        slider.setProgress(current - minimum);
        slider.setContentDescription(title);
        slider.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar bar, int progress, boolean fromUser) {
                value.setText(format.apply(minimum + progress));
            }

            @Override public void onStartTrackingTouch(SeekBar bar) {}

            // Saving on release rather than on every pixel of the drag: each write takes the file
            // lock the input service also takes.
            @Override public void onStopTrackingTouch(SeekBar bar) {
                onCommit.accept(minimum + bar.getProgress());
            }
        });
        sheet.add(slider);
    }
}
