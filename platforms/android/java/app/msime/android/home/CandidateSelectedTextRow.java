package app.msime.android.home;

import android.text.InputType;
import androidx.fragment.app.Fragment;
import app.msime.android.CandidateSelectedTextPolicy;
import app.msime.android.JsonPolicy;
import org.json.JSONObject;

/**
 * 「候选选中文字色」一行（#10740）：共享偏好 `custom_theme.candidate_colors.selected_text`，与 Windows、iOS、macOS 和共享设置页的同名取色器是同一个字段。
 *
 * <p>Android 没有系统取色器，这里给跟随主题、几种常用颜色和「自定义…」输入 `#RRGGBB`。选了颜色就选上自定义主题，规则和补丁由共享 Rust 计算；值由共享偏好校验把关，非法值保存失败。
 */
final class CandidateSelectedTextRow {
    private static final String TITLE = "候选选中文字色";

    private CandidateSelectedTextRow() {}

    /** 在 `group` 里加这一行；`preferences` 是页面当前读到的偏好，`reload` 在保存成功或失败后重读页面。 */
    static GroupCard.Row add(GroupCard group, Fragment page, JSONObject preferences, Runnable reload) {
        String current = picked(preferences);
        return group.nav(TITLE, null, CandidateSelectedTextPolicy.label(current),
            () -> pick(page, current, reload))
            .help("首选候选文字的颜色，选了颜色会切到自定义主题；跟随主题时候选栏照旧用键盘皮肤的颜色");
    }

    private static String picked(JSONObject preferences) {
        JSONObject custom = preferences == null ? null : preferences.optJSONObject("custom_theme");
        JSONObject colors = custom == null ? null : custom.optJSONObject("candidate_colors");
        return CandidateSelectedTextPolicy.normalized(
            colors == null ? null : JsonPolicy.strictString(colors.opt(CandidateSelectedTextPolicy.SLOT)));
    }

    private static void pick(Fragment page, String current, Runnable reload) {
        OptionSheet sheet = new OptionSheet(page.requireContext(), TITLE, null);
        sheet.option("跟随主题", current == null, () -> save(page, null, reload));
        boolean preset = false;
        for (CandidateSelectedTextPolicy.Preset option : CandidateSelectedTextPolicy.PRESETS) {
            boolean selected = option.value().equals(current);
            preset |= selected;
            sheet.option(option.label(), selected, () -> save(page, option.value(), reload));
        }
        sheet.option("自定义…", current != null && !preset, () -> custom(page, current, reload));
        sheet.show();
    }

    private static void custom(Fragment page, String current, Runnable reload) {
        InputDialog dialog = new InputDialog(page.requireContext(), TITLE, "输入 #RRGGBB，例如 #7B3FBF");
        dialog.addField("#RRGGBB", current, InputType.TYPE_CLASS_TEXT);
        dialog.setValidator(values -> CandidateSelectedTextPolicy.normalized(values.get(0).trim()) != null);
        dialog.setPrimary("保存", values -> save(page, CandidateSelectedTextPolicy.normalized(values.get(0).trim()), reload));
        dialog.show();
    }

    /** 在最新文档上请求并应用共享补丁，桥接失败时沿用原保存失败流程。 */
    private static void save(Fragment page, String value, Runnable reload) {
        KeyboardSheets.save(page, preferences -> CandidateSelectedTextUpdate.apply(preferences, value), reload, reload);
    }
}
