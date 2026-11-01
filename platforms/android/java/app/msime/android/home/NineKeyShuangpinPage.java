package app.msime.android.home;

import android.content.Context;
import android.net.Uri;
import android.text.InputType;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.AppEdition;
import app.msime.android.HttpBodyPolicy;
import app.msime.android.KeyboardScheme;
import app.msime.android.NativeClient;
import app.msime.android.NineKeyShuangpinKeypad;
import app.msime.android.SchemePreferences;
import app.msime.android.TextPolicy;
import app.msime.android.core.InputViewValuePolicy;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 九键双拼页（#6572）：选用哪张表（米旮旯 3、米旮旯 2、辜氏 2 低重码或自定义），看当前表每个数字键上的声母和韵母，编辑、导入、导出自定义表。
 *
 * <p>表的内容、校验和键面都来自 host-api 的 `msime_client_nine_key_shuangpin_layout`（{@link NativeClient#nineKeyShuangpinLayout}），本页不另存键位也不自己判断表合不合法：编辑中的表每改一格就问一次，问题带着出错的单位显示在表下面，能用时才能保存。自定义表存在共享偏好 `nine_key_shuangpin_custom_profile`，形状与 26 键的 `shuangpin_custom_profile` 相同，键是数字；导入导出的文件就是这个 JSON。
 */
public final class NineKeyShuangpinPage extends ReloadingDetailPage {
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("方案", "键位", "自定义");
    }

    /** 导入文件的上限：一张表只有六十多个单位，几 KB；与 JNI 请求的上限一致。 */
    private static final int MAX_IMPORT_BYTES = 16 * 1024;
    private static final List<String> PARTS = List.of("initials", "finals", "zero_initials");

    @Nullable private JSONObject preferences;
    /** 编辑中的自定义表；第一次打开页面时取保存着的表，没有时从米旮旯 3 的表开始。离开页面前不保存就丢掉。 */
    @Nullable private JSONObject draft;
    private boolean draftChanged;

    private final ActivityResultLauncher<String[]> openDocument =
        registerForActivityResult(new ActivityResultContracts.OpenDocument(), this::onImportPicked);
    private final ActivityResultLauncher<String> createDocument =
        registerForActivityResult(new ActivityResultContracts.CreateDocument("application/json"), this::onExportPicked);

    @Override protected void onContentReady() {
        if (preferences != null) render();
    }

    @Override protected boolean shouldReloadWhenVisible() {
        // 正在编辑的表不能因为从文件选择器回来就被保存着的那份盖掉。
        return !draftChanged;
    }

    @Override protected void reload() {
        HostTask.run(this, KeyboardSheets::preferences, loaded -> {
            if (loaded == null) {
                MsToast.show(requireContext(), "读不出设置，请稍后重试");
                return;
            }
            preferences = loaded;
            if (draft == null || !draftChanged) draft = initialDraft(loaded);
            render();
        });
    }

    private String profile() {
        return NineKeyShuangpinKeypad.normalizedProfile(
            InputViewValuePolicy.textOr(preferences, "nine_key_shuangpin_profile", NineKeyShuangpinKeypad.DEFAULT_PROFILE));
    }

    /** 保存着的自定义表；没有时是米旮旯 3 的表，作为编辑的起点。 */
    private static JSONObject initialDraft(JSONObject preferences) {
        JSONObject saved = preferences.optJSONObject("nine_key_shuangpin_custom_profile");
        if (saved != null && saved.length() > 0) return copy(saved);
        JSONObject builtin = layout(NineKeyShuangpinKeypad.DEFAULT_PROFILE, null);
        JSONObject table = builtin == null ? null : builtin.optJSONObject("table");
        return table == null ? emptyTable() : copy(table);
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null || preferences == null || draft == null) return;
        target.removeAllViews();
        String profile = profile();
        JSONObject savedTable = preferences.optJSONObject("nine_key_shuangpin_custom_profile");
        JSONObject savedLayout = savedTable == null ? null : layout(NineKeyShuangpinKeypad.CUSTOM_PROFILE, savedTable);
        boolean savedUsable = savedLayout != null && savedLayout.isNull("problem");

        AppEdition edition = AppEdition.current();
        boolean applied = SchemePreferences.storedScheme(preferences, edition) == KeyboardScheme.NINE_KEY_SHUANGPIN;
        GroupCard choice = GroupCard.add(sectionColumn(target, "方案"), "方案");
        if (!applied && KeyboardScheme.NINE_KEY_SHUANGPIN.offeredBy(edition)) {
            choice.button("使用九键双拼键盘", null, "切换", this::useKeyboard).help("键盘切换到九键双拼，按下面选的表打字");
        }
        for (String option : NineKeyShuangpinKeypad.BUILTIN_PROFILES) {
            choice.nav(NineKeyShuangpinKeypad.profileTitle(option), null, option.equals(profile) ? "使用中" : null,
                () -> chooseProfile(option));
        }
        // 还没有能用的自定义表是这一行点不了的原因，留在副标题；能用时的说明收进「?」。
        choice.nav("自定义", savedUsable ? null : "先在下面编辑并保存一张自定义表",
            NineKeyShuangpinKeypad.CUSTOM_PROFILE.equals(profile) ? "使用中" : null,
            savedUsable ? () -> chooseProfile(NineKeyShuangpinKeypad.CUSTOM_PROFILE) : null).help("用下面保存的自定义表");

        JSONObject current = layout(profile, savedTable);
        GroupCard keys = GroupCard.add(sectionColumn(target, "键位"), "键位（" + NineKeyShuangpinKeypad.profileTitle(
            current == null ? profile : current.optString("effective", profile)) + "）");
        renderKeys(keys, current);
        if (current != null && !current.isNull("problem")) {
            keys.footer("保存着的自定义表不能用，键盘按米旮旯 3 打字：" + problemText(current.optJSONObject("problem")));
        }

        GroupCard editor = GroupCard.add(sectionColumn(target, "自定义"), "自定义表");
        editor.help("每个音节两位数字：先按声母所在的键，再按韵母所在的键；零声母音节直接给两位数字。一个编码可以对应好几个音节，数字 0 到 9 都可以用。");
        editor.button("从内置方案开始", "用一张内置的表替换正在编辑的表", "选择", this::startFromBuiltin);
        editor.button("导入", null, "导入", () -> openDocument.launch(
            new String[] {"application/json", "text/plain", "application/octet-stream"})).help("从 JSON 文件读入一张表");
        editor.button("导出", null, "导出",
            () -> createDocument.launch("九键双拼键位.json")).help("把正在编辑的表存成 JSON 文件");
        JSONObject draftLayout = layout(NineKeyShuangpinKeypad.CUSTOM_PROFILE, draft);
        JSONObject problem = draftLayout == null ? null : draftLayout.optJSONObject("problem");
        String saveSubtitle = problem != null ? problemText(problem)
            : draftChanged ? "表可以用，还没有保存" : "表可以用";
        GroupCard.Row save = editor.button("保存并使用", saveSubtitle, "保存", this::saveDraft);
        save.setEnabled(problem == null);
        for (String part : PARTS) {
            GroupCard units = GroupCard.add(sectionColumn(target, "自定义"), NineKeyShuangpinKeypad.partTitle(part));
            JSONObject entries = draft.optJSONObject(part);
            if (entries == null) continue;
            for (Iterator<String> names = sortedKeys(entries); names.hasNext(); ) {
                String unit = names.next();
                String key = entries.optString(unit, "");
                boolean wrong = problem != null && part.equals(problem.optString("part"))
                    && unit.equals(problem.optString("unit"));
                units.nav(NineKeyShuangpinKeypad.unitLabel(unit), wrong ? problemText(problem) : null, key,
                    () -> editUnit(part, unit, key));
            }
        }
    }

    private static void renderKeys(GroupCard keys, @Nullable JSONObject layout) {
        org.json.JSONArray faces = layout == null ? null : layout.optJSONArray("keys");
        if (faces == null || faces.length() == 0) {
            keys.note("取不到这张表的键位。");
            return;
        }
        for (int index = 0; index < faces.length(); index++) {
            JSONObject face = faces.optJSONObject(index);
            if (face == null) continue;
            String initials = join(face.optJSONArray("initials"));
            if (face.optBoolean("zero_initial", false)) initials = initials.isEmpty() ? "零声母" : initials + " 零声母";
            String finals = join(face.optJSONArray("finals"));
            keys.value(face.optString("key", ""), initials.isEmpty() ? null : "声母 " + initials,
                finals.isEmpty() ? null : finals);
        }
    }

    private static String join(@Nullable org.json.JSONArray values) {
        if (values == null) return "";
        List<String> parts = new ArrayList<>(values.length());
        for (int index = 0; index < values.length(); index++) parts.add(values.optString(index, ""));
        return String.join(" ", parts);
    }

    private static String problemText(@Nullable JSONObject problem) {
        if (problem == null) return "";
        return NineKeyShuangpinKeypad.problemMessage(problem.optString("kind", ""), problem.optString("part", ""),
            problem.optString("unit", ""), problem.optString("key", ""));
    }

    private void editUnit(String part, String unit, String key) {
        boolean pair = "zero_initials".equals(part);
        InputDialog dialog = new InputDialog(requireContext(),
            NineKeyShuangpinKeypad.partTitle(part) + " " + NineKeyShuangpinKeypad.unitLabel(unit),
            pair ? "两位数字，例如 22" : "一位数字，0 到 9");
        dialog.addField(pair ? "两位数字" : "数字", key, InputType.TYPE_CLASS_NUMBER);
        dialog.setValidator(values -> NineKeyShuangpinKeypad.validKey(part, TextPolicy.trimmed(values.get(0))));
        dialog.setPrimary("确定", values -> {
            try {
                if (draft != null) draft.getJSONObject(part).put(unit, TextPolicy.trimmed(values.get(0)));
            } catch (JSONException ignored) {
                return;
            }
            draftChanged = true;
            render();
        });
        dialog.show();
    }

    private void startFromBuiltin() {
        OptionSheet sheet = new OptionSheet(requireContext(), "从内置方案开始", "正在编辑的表会被替换");
        for (String option : NineKeyShuangpinKeypad.BUILTIN_PROFILES) {
            sheet.option(NineKeyShuangpinKeypad.profileTitle(option), false, () -> {
                JSONObject builtin = layout(option, null);
                JSONObject table = builtin == null ? null : builtin.optJSONObject("table");
                if (table == null) {
                    MsToast.show(requireContext(), "取不到这张表");
                    return;
                }
                draft = copy(table);
                draftChanged = true;
                render();
            });
        }
        sheet.show();
    }

    private void saveDraft() {
        JSONObject table = draft == null ? null : copy(draft);
        JSONObject checked = table == null ? null : layout(NineKeyShuangpinKeypad.CUSTOM_PROFILE, table);
        if (checked == null || !checked.isNull("problem")) {
            MsToast.show(requireContext(), checked == null ? "这张表不能用" : problemText(checked.optJSONObject("problem")));
            return;
        }
        savePreferences(values -> {
            values.put("nine_key_shuangpin_custom_profile", table);
            values.put("nine_key_shuangpin_profile", NineKeyShuangpinKeypad.CUSTOM_PROFILE);
        }, () -> {
            draftChanged = false;
            MsToast.show(requireContext(), "已保存，九键双拼使用自定义表");
        });
    }

    private void chooseProfile(String profile) {
        savePreferences(values -> values.put("nine_key_shuangpin_profile", profile));
    }

    private void useKeyboard() {
        HostTask.run(this, context -> {
            JSONObject pending = SchemePreferences.withScheme(HostStore.loadPreferences(context),
                KeyboardScheme.NINE_KEY_SHUANGPIN, null);
            return pending == null ? null : HostStore.savePreferences(context, pending);
        }, saved -> {
            if (saved == null) MsToast.show(requireContext(), "切换失败，保留当前方案");
            reload();
        });
    }

    // ---- 导入导出 ----

    private void onImportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null) return;
        HostTask.run(this, context -> readTable(context, uri), table -> {
            if (table == null) {
                MsToast.show(requireContext(), "读不出这个文件，请选导出的九键双拼表（JSON）");
                return;
            }
            draft = table;
            draftChanged = true;
            JSONObject checked = layout(NineKeyShuangpinKeypad.CUSTOM_PROFILE, table);
            MsToast.show(requireContext(), checked != null && checked.isNull("problem")
                ? "已导入，保存后生效" : "已导入，表里有要改的地方");
            render();
        });
    }

    /** 读出文件里的表：只认 `{initials, finals, zero_initials}` 三个对象，值都是字符串；别的形状返回 null。 */
    @Nullable private static JSONObject readTable(Context context, Uri uri) {
        try (InputStream input = context.getContentResolver().openInputStream(uri)) {
            if (input == null) return null;
            byte[] bytes = HttpBodyPolicy.readBounded(input, MAX_IMPORT_BYTES);
            if (bytes == null) return null;
            JSONObject parsed = new JSONObject(TextPolicy.utf8(bytes));
            JSONObject table = emptyTable();
            for (Iterator<String> parts = parsed.keys(); parts.hasNext(); ) {
                String part = parts.next();
                JSONObject entries = parsed.optJSONObject(part);
                if (!PARTS.contains(part) || entries == null) return null;
                for (Iterator<String> units = entries.keys(); units.hasNext(); ) {
                    String unit = units.next();
                    Object key = entries.opt(unit);
                    if (!(key instanceof String text)) return null;
                    table.getJSONObject(part).put(unit, text);
                }
            }
            return table;
        } catch (IOException | JSONException | SecurityException error) {
            return null;
        }
    }

    private void onExportPicked(@Nullable Uri uri) {
        if (uri == null || getView() == null || draft == null) return;
        String text;
        try {
            text = draft.toString(2);
        } catch (JSONException error) {
            MsToast.show(requireContext(), "导出失败");
            return;
        }
        HostTask.run(this, context -> {
            try (OutputStream output = context.getContentResolver().openOutputStream(uri, "wt")) {
                if (output == null) return "写入文件失败，请重新选择位置。";
                output.write(TextPolicy.utf8Bytes(text + "\n"));
                return "已导出";
            } catch (IOException | SecurityException error) {
                return "写入文件失败，请重新选择位置。";
            }
        }, message -> MsToast.show(requireContext(), message));
    }

    // ---- 表 ----

    /** host-api 给出的方案样子（`effective`、`table`、`problem`、`keys`）；`table` 只在 `custom` 时带上。取不到时为 null。 */
    @Nullable private static JSONObject layout(String profile, @Nullable JSONObject table) {
        try {
            JSONObject request = new JSONObject().put("profile", profile);
            if (table != null) request.put("table", table);
            JSONObject response = new JSONObject(NativeClient.nineKeyShuangpinLayout(request.toString()));
            return response.optBoolean("ok", false) ? response.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    private static JSONObject emptyTable() {
        JSONObject table = new JSONObject();
        try {
            for (String part : PARTS) table.put(part, new JSONObject());
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        return table;
    }

    private static JSONObject copy(JSONObject value) {
        try {
            return new JSONObject(value.toString());
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static Iterator<String> sortedKeys(JSONObject object) {
        List<String> names = new ArrayList<>(object.length());
        for (Iterator<String> keys = object.keys(); keys.hasNext(); ) names.add(keys.next());
        java.util.Collections.sort(names);
        return names.iterator();
    }
}
