package app.msime.android.home;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.text.InputType;
import android.view.Gravity;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.SeekBar;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import androidx.fragment.app.Fragment;
import app.msime.android.AiPolishConfiguration;
import app.msime.android.AppEdition;
import app.msime.android.InputFeatureToggle;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardSkin;
import app.msime.android.R;
import app.msime.android.TextPolicy;
import app.msime.android.core.Telemetry;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.textfield.TextInputEditText;
import com.google.android.material.textfield.TextInputLayout;
import java.util.List;
import java.util.function.Consumer;
import org.json.JSONArray;
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

    /**
     * 主题：颜色模式，以及共享目录里的全局主题——跟随系统、五套内置主题和自定义主题。
     *
     * <p>The design's 主题 page as a sheet: its 颜色模式 control and its grid of theme cards (dc.html L2003 and L771-781), each card a swatch of the theme's own background, candidate panel, accent and text. Built-in cards draw the catalog's `preview` colours, which is what the contract gives them for; 跟随系统 draws the Material 3 keyboard in the mode the keyboard is in; the custom card draws the custom theme as it stands.
     */
    public static void showSkins(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        String current = preferences.optString("global_theme", "system");
        SettingsSheet sheet = new SettingsSheet(context, "主题",
            "全局主题同时决定键盘和候选栏的配色，只改外观，不改输入方案。");
        TextView status = sheet.addStatus();

        // 颜色模式 is the `theme` preference: the keyboard's light or dark, and this app's too.
        sheet.addHeading("颜色模式");
        String[][] modes = {{AppMode.SYSTEM, "跟随系统"}, {AppMode.LIGHT, "浅色"}, {AppMode.DARK, "深色"}};
        String mode = AppMode.of(preferences);
        MaterialButtonToggleGroup group = new MaterialButtonToggleGroup(context);
        group.setSingleSelection(true);
        group.setSelectionRequired(true);
        LayoutInflater inflater = LayoutInflater.from(context);
        int[] segments = new int[modes.length];
        for (int index = 0; index < modes.length; index++) {
            MaterialButton segment = (MaterialButton) inflater.inflate(R.layout.item_segment, group, false);
            segment.setId(View.generateViewId());
            segment.setText(modes[index][1]);
            group.addView(segment);
            segments[index] = segment.getId();
            if (modes[index][0].equals(mode)) group.check(segment.getId());
        }
        group.addOnButtonCheckedListener((ignored, id, checked) -> {
            if (!checked) return;
            for (int index = 0; index < segments.length; index++) {
                if (segments[index] != id || modes[index][0].equals(mode)) continue;
                // The sheet closes with the save: a new mode recreates the activity under it.
                save(fragment, snapshot, "theme", modes[index][0], status, sheet, changed);
            }
        });
        LinearLayout.LayoutParams groupParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        groupParams.topMargin = dp(context, 6);
        sheet.content().addView(group, groupParams);

        sheet.addHeading("皮肤");
        boolean dark = KeyboardSkin.resolveDark(
            preferences.optString("screen_keyboard_theme", "follow"),
            preferences.optString("theme", "system"), AppMode.dark(context));
        JSONArray themes = HostStore.themeCatalog();
        LinearLayout row = null;
        int placed = 0;
        for (int index = 0; index < themes.length(); index++) {
            JSONObject entry = themes.optJSONObject(index);
            if (entry == null) continue;
            String id = entry.optString("id", "");
            if (id.isEmpty()) continue;
            String title = entry.optString("title", id);
            int[] swatch = swatch(entry, preferences, dark);
            if (placed % 2 == 0) {
                row = new LinearLayout(context);
                row.setOrientation(LinearLayout.HORIZONTAL);
                row.setBaselineAligned(false);
                LinearLayout.LayoutParams rowParams = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
                rowParams.topMargin = dp(context, placed == 0 ? 6 : 12);
                sheet.content().addView(row, rowParams);
            }
            LinearLayout.LayoutParams cardParams = new LinearLayout.LayoutParams(0,
                ViewGroup.LayoutParams.WRAP_CONTENT, 1);
            if (placed % 2 == 1) cardParams.setMarginStart(dp(context, 12));
            row.addView(themeCard(context, title, swatch, id.equals(current),
                () -> save(fragment, snapshot, "global_theme", id, status, sheet, changed)), cardParams);
            placed++;
        }
        // An odd count leaves the last card half a row wide, as the design's grid does.
        if (row != null && placed % 2 == 1) {
            LinearLayout.LayoutParams spacer = new LinearLayout.LayoutParams(0, 0, 1);
            spacer.setMarginStart(dp(context, 12));
            row.addView(new View(context), spacer);
        }
        if (placed == 0) status.setText("主题列表不可用");
        sheet.addNote("自定义主题的底色、候选颜色和我的皮肤在键盘的皮肤面板里编辑；这里只切换。");
        sheet.show();
    }

    /**
     * One card's four colours -- background, candidate panel, accent, text -- as the design's themeCards reads them (dc.html L2128).
     *
     * <p>A built-in theme has them in the catalog's `preview`. `system` and `custom` carry no preview there, because what they look like depends on this device: 跟随系统 is the Material 3 keyboard in the keyboard's current mode, and the custom card is the custom theme resolved as it stands, the same way the keyboard resolves it.
     */
    private static int[] swatch(JSONObject entry, JSONObject preferences, boolean dark) {
        JSONObject preview = entry.optJSONObject("preview");
        KeyboardSkin fallback = KeyboardSkin.system(dark);
        if (preview != null) {
            return new int[] {
                contractColor(preview, "background", fallback.background()),
                contractColor(preview, "panel", fallback.keyBackground()),
                contractColor(preview, "accent", fallback.accent()),
                contractColor(preview, "text", fallback.keyForeground()),
            };
        }
        KeyboardSkin skin = fallback;
        if ("custom".equals(entry.optString("id"))) {
            try {
                JSONObject custom = new JSONObject(preferences.toString());
                custom.put("global_theme", "custom");
                skin = HostStore.keyboardSkin(custom, dark);
            } catch (JSONException error) {
                // The card falls back to the Material 3 swatch; selecting it still works.
            }
        }
        return new int[] {
            Color.parseColor(skin.background()), Color.parseColor(skin.keyBackground()),
            Color.parseColor(skin.accent()), Color.parseColor(skin.keyForeground()),
        };
    }

    /** A `#RRGGBB` or `#RRGGBBAA` catalog colour as an Android colour, or the fallback (an Android colour string) when the slot is missing or unreadable. */
    private static int contractColor(JSONObject preview, String key, String fallback) {
        String value = preview.isNull(key) ? null : KeyboardSkin.androidColor(preview.optString(key, null));
        return Color.parseColor(value == null ? fallback : value);
    }

    /** The design's theme card: a 72dp swatch with a candidate panel on it, then the name and 使用中 under it, a 2dp accent ring when chosen. */
    private static View themeCard(Context context, String title, int[] swatch, boolean selected,
            Runnable onPick) {
        LinearLayout card = new LinearLayout(context);
        card.setOrientation(LinearLayout.VERTICAL);
        int padding = dp(context, 8);
        card.setPadding(padding, padding, padding, padding);
        GradientDrawable face = new GradientDrawable();
        face.setCornerRadius(dp(context, 16));
        // The sheet itself is the surface colour, so the cards take the page colour to stand off it.
        face.setColor(ContextCompat.getColor(context, R.color.mist));
        if (selected) face.setStroke(dp(context, 2), ContextCompat.getColor(context, R.color.forest));
        card.setBackground(face);
        card.setClipToOutline(true);
        int ripple = rippleBackground(context);
        if (ripple != 0) card.setForeground(ContextCompat.getDrawable(context, ripple));
        card.setClickable(true);
        card.setFocusable(true);
        card.setContentDescription(title + (selected ? "，使用中" : ""));
        card.setOnClickListener(ignored -> onPick.run());

        android.widget.FrameLayout field = new android.widget.FrameLayout(context);
        GradientDrawable ground = new GradientDrawable();
        ground.setCornerRadius(dp(context, 6));
        ground.setColor(swatch[0]);
        field.setBackground(ground);
        field.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        LinearLayout panel = new LinearLayout(context);
        panel.setOrientation(LinearLayout.HORIZONTAL);
        panel.setPadding(dp(context, 9), dp(context, 5), dp(context, 9), dp(context, 5));
        GradientDrawable plate = new GradientDrawable();
        plate.setCornerRadius(dp(context, 4));
        plate.setColor(swatch[1]);
        panel.setBackground(plate);
        panel.setElevation(dp(context, 2));
        TextView first = new TextView(context);
        first.setText("1 候选");
        first.setTextSize(13);
        first.setMaxLines(1);
        first.setTextColor(swatch[2]);
        panel.addView(first);
        TextView second = new TextView(context);
        second.setText("2 侯选");
        second.setTextSize(13);
        second.setMaxLines(1);
        second.setTextColor(swatch[3]);
        LinearLayout.LayoutParams secondParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        secondParams.setMarginStart(dp(context, 8));
        panel.addView(second, secondParams);
        field.addView(panel, new android.widget.FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.CENTER));
        card.addView(field, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, dp(context, 72)));

        LinearLayout footer = new LinearLayout(context);
        footer.setOrientation(LinearLayout.HORIZONTAL);
        footer.setGravity(Gravity.CENTER_VERTICAL);
        footer.setPadding(dp(context, 2), 0, dp(context, 2), 0);
        TextView name = new TextView(context);
        name.setText(title);
        name.setTextSize(13);
        name.setMaxLines(1);
        name.setEllipsize(android.text.TextUtils.TruncateAt.END);
        name.setTextColor(ContextCompat.getColor(context, R.color.ink));
        footer.addView(name, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        if (selected) {
            TextView badge = new TextView(context);
            badge.setText("使用中");
            badge.setTextSize(12);
            badge.setTextColor(ContextCompat.getColor(context, R.color.forest));
            footer.addView(badge);
        }
        LinearLayout.LayoutParams footerParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        footerParams.topMargin = dp(context, 8);
        card.addView(footer, footerParams);
        return card;
    }

    /** 输入方案：一次切换要同时写四个键，交给共享的映射算，不在这里拼。 */
    public static void showSchemes(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        AppEdition edition = AppEdition.current();
        KeyboardScheme current = KeyboardScheme.fromPreferences(
            preferences.optString("scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"),
            preferences.optString("touch_keyboard_layout", "twenty_six_key"), edition);
        SettingsSheet sheet = new SettingsSheet(context, "输入方案",
            "换方案会同时换掉键盘布局。已经学到的词不受影响。");
        TextView status = sheet.addStatus();
        // Cantonese, Zhuyin and Stroke are offered only once their dictionary is installed: without it host-api falls back from them, so picking one would change nothing.
        String languageDictionaries = HostStore.languageDictionaries(context);
        String currentWubi = KeyboardScheme.normalizedWubiProfile(
            preferences.optString("wubi_profile", KeyboardScheme.WUBI_86));
        for (KeyboardScheme scheme : KeyboardScheme.values()) {
            // 本版本没有的方案不列：五笔版只有五笔和手写，拼音版没有五笔和各语言方案。
            if (!scheme.offeredBy(edition) || !scheme.installed(languageDictionaries)) continue;
            if (scheme == KeyboardScheme.WUBI) {
                // 五笔只有一个方案，86 与 98 是它的两个版本：各列一行，选中哪行就同时写 `scheme` 和 `wubi_profile`。
                for (String profile : List.of(KeyboardScheme.WUBI_86, KeyboardScheme.WUBI_98)) {
                    sheet.add(choice(context, scheme.title(profile),
                        scheme.glyph() + " · " + scheme.badge(profile),
                        scheme == current && profile.equals(currentWubi),
                        () -> applyScheme(fragment, snapshot, scheme, profile, status, sheet, changed)));
                }
                continue;
            }
            sheet.add(choice(context, scheme.title(), scheme.glyph() + " · " + scheme.badge(),
                scheme == current,
                () -> applyScheme(fragment, snapshot, scheme, null, status, sheet, changed)));
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
            KeyboardGeometry.keySpacing(KeyboardGeometry.strictInt(
                preferences, "touch_key_spacing_tenths", -1)),
            KeyboardGeometry::display,
            value -> save(fragment, snapshot, "touch_key_spacing_tenths", value, status, null,
                changed));
        slider(context, sheet, "行间距",
            KeyboardGeometry.MIN_ROW_SPACING_TENTHS, KeyboardGeometry.MAX_ROW_SPACING_TENTHS,
            KeyboardGeometry.rowSpacing(KeyboardGeometry.strictInt(
                preferences, "touch_row_spacing_tenths", -1)),
            KeyboardGeometry::display,
            value -> save(fragment, snapshot, "touch_row_spacing_tenths", value, status, null,
                changed));
        slider(context, sheet, "键盘高度",
            KeyboardGeometry.MIN_HEIGHT_ADJUSTMENT_DP, KeyboardGeometry.MAX_HEIGHT_ADJUSTMENT_DP,
            KeyboardGeometry.heightAdjustment(
                KeyboardGeometry.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE)),
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
                    value -> {
                        save(fragment, snapshot, feature.key(), value, status, null, changed);
                        // Off has to stop reporting now, not on the next start: the queue is cleared at once.
                        if (feature == InputFeatureToggle.USAGE_REPORTING) Telemetry.setEnabled(context, value);
                    }));
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
     *
     * <p>Like the other sheets there is no save button: the switch saves when flipped, and a text field saves when it loses focus, when its IME action is pressed, and when the sheet closes. Text is not saved while it is being typed, because a half-typed endpoint is another origin and the credential would be filed under it.
     */
    public static void showAi(Fragment fragment, JSONObject snapshot, Runnable changed) {
        Context context = fragment.requireContext();
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return;
        JSONObject ai = preferences.optJSONObject("ai_assistant");
        SettingsSheet sheet = new SettingsSheet(context, "AI 润色与回复",
            "由你自己的服务端提供。凭据只保存在本机，按端点的来源分别存放。改动自动保存。");
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
        // The sheet edits the slot `prompt_id` selects, the one the keyboard reads.
        String promptKey = AiPolishConfiguration.promptSlotKey(
            ai == null ? "" : ai.optString("prompt_id", ""));
        TextInputLayout prompt = field(context, "润色提示词",
            ai == null ? "" : ai.optString(promptKey, ""), InputType.TYPE_CLASS_TEXT);
        List<TextInputLayout> fields = List.of(endpoint, model, token, prompt);
        for (TextInputLayout entry : fields) sheet.add(entry);

        // What was last written (or what the sheet opened with), so a focus change or a close with nothing edited writes nothing. `base` is the object the next write starts from: the last one written, so a credential this sheet filed under an earlier origin is kept.
        String[] committed = {aiInputs(enabled.isChecked(), text(endpoint), text(model),
            text(prompt), text(token))};
        JSONObject[] base = {ai};
        Runnable commit = () -> {
            String inputs = aiInputs(enabled.isChecked(), text(endpoint), text(model),
                text(prompt), text(token));
            if (inputs.equals(committed[0])) return;
            JSONObject next = buildAi(base[0], enabled.isChecked(), text(endpoint), text(model),
                promptKey, text(prompt), text(token));
            if (next == null) {
                // An endpoint the keyboard would refuse is not written; the stored one stays in force until this is corrected.
                endpoint.setError("端点必须是一个 https 地址");
                status.setText("端点必须是一个 https 地址");
                return;
            }
            endpoint.setError(null);
            committed[0] = inputs;
            base[0] = next;
            save(fragment, snapshot, "ai_assistant", next, status, null, changed);
        };
        enabled.setOnCheckedChangeListener((button, checked) -> commit.run());
        for (TextInputLayout entry : fields) {
            if (entry.getEditText() == null) continue;
            entry.getEditText().setOnFocusChangeListener((view, focused) -> {
                if (!focused) commit.run();
            });
            entry.getEditText().setOnEditorActionListener((view, action, event) -> {
                commit.run();
                return false;
            });
        }
        sheet.setOnDismiss(commit);
        sheet.addNote("端点必须是 HTTPS。留空凭据表示该来源不需要凭据，不会清掉其他来源已存的凭据。");
        sheet.show();
    }

    /** The sheet's inputs as one comparable value, to tell whether anything changed since the last write. */
    private static String aiInputs(boolean enabled, String endpoint, String model, String prompt,
            String token) {
        return new JSONArray(List.of(enabled, endpoint, model, prompt, token)).toString();
    }

    @Nullable private static JSONObject buildAi(@Nullable JSONObject previous, boolean enabled,
            String endpoint, String model, String promptKey, String prompt, String token) {
        if (enabled && !TextPolicy.validAuthority(endpoint, "https://", 2048)) return null;
        try {
            JSONObject next = previous == null ? new JSONObject()
                : new JSONObject(previous.toString());
            next.put("enabled", enabled);
            next.put("endpoint", endpoint);
            next.put("model", model);
            next.put(promptKey, prompt);
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
            return app.msime.android.AiPolishConfiguration.credentialOrigin(endpoint);
        } catch (RuntimeException error) {
            return "";
        }
    }

    /**
     * A copy of the preference snapshot switched to one scheme: the four keys a scheme change writes together, computed by the shared mapping rather than assembled by each caller, plus the keyboard picker's `selected` when that list exists. Also used by onboarding's scheme step, so both write the same thing.
     *
     * @return the edited copy, or null when the snapshot has no preferences object
     */
    @Nullable static JSONObject withScheme(JSONObject snapshot, KeyboardScheme scheme) {
        return withScheme(snapshot, scheme, null);
    }

    /**
     * 同上，另外把 `wubi_profile` 写成 `wubiProfile`；传 null 保留现有版本，切到五笔时沿用用户上次选的 86 或 98。
     *
     * @return 改好的副本；快照里没有 preferences 对象时为 null
     */
    @Nullable static JSONObject withScheme(JSONObject snapshot, KeyboardScheme scheme,
            @Nullable String wubiProfile) {
        JSONObject preferences = preferences(snapshot);
        if (preferences == null) return null;
        AppEdition edition = AppEdition.current();
        KeyboardScheme.PreferenceMapping mapping = scheme.mapping(
            preferences.optString("last_chinese_scheme", edition.defaultScheme()),
            preferences.optString("shuangpin_profile", "xiaohe"), edition);
        try {
            JSONObject pending = new JSONObject(snapshot.toString());
            JSONObject values = pending.getJSONObject("preferences");
            values.put("scheme", mapping.scheme());
            values.put("last_chinese_scheme", mapping.lastChineseScheme());
            values.put("shuangpin_profile", mapping.shuangpinProfile());
            values.put("touch_keyboard_layout", mapping.touchKeyboardLayout());
            if (wubiProfile != null) {
                values.put("wubi_profile", KeyboardScheme.normalizedWubiProfile(wubiProfile));
            }
            // Once the keyboard's own picker has written its scheme list, its `selected` outranks `scheme` when the keyboard resolves what to show (KeyboardScheme.resolveEnabledSelection), so a switch made here has to move it too, and enable the scheme if the list left it out. Without the list the keyboard follows `scheme` alone; do not create one.
            JSONObject schemes = values.optJSONObject("touch_keyboard_schemes");
            if (schemes != null) {
                JSONArray enabled = schemes.optJSONArray("enabled");
                if (enabled == null) {
                    enabled = new JSONArray();
                    schemes.put("enabled", enabled);
                }
                boolean listed = false;
                for (int index = 0; index < enabled.length(); index++) {
                    if (scheme.preferenceId().equals(enabled.isNull(index) ? null : enabled.optString(index, null))) {
                        listed = true;
                        break;
                    }
                }
                if (!listed) enabled.put(scheme.preferenceId());
                schemes.put("selected", scheme.preferenceId());
            }
            return pending;
        } catch (JSONException error) {
            return null;
        }
    }

    private static void applyScheme(Fragment fragment, JSONObject snapshot, KeyboardScheme scheme,
            @Nullable String wubiProfile, TextView status, SettingsSheet sheet, Runnable changed) {
        if (preferences(snapshot) == null) return;
        JSONObject pending = withScheme(snapshot, scheme, wubiProfile);
        if (pending == null) {
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

    private static int dp(Context context, int value) {
        return Math.round(value * context.getResources().getDisplayMetrics().density);
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
