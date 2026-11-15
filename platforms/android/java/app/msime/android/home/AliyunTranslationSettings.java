package app.msime.android.home;

import android.text.InputType;
import android.widget.LinearLayout;
import app.msime.android.AliyunTranslationPolicy;
import app.msime.android.JsonPolicy;
import org.json.JSONArray;
import org.json.JSONObject;

/** 阿里云原生设置表单；选项、默认值和字段校验只使用共享目录。 */
final class AliyunTranslationSettings {
    private AliyunTranslationSettings() {}

    static void render(TranslationServicePage page, LinearLayout column, JSONObject config) {
        GroupCard group = GroupCard.add(column, "阿里云机器翻译");
        JSONObject catalog = AliyunTranslationPolicy.catalog();
        if (catalog == null) { group.note("共享服务目录暂不可用，请稍后重试"); return; }
        group.nav("AccessKey ID", null, filled(config, "access_key_id"), () -> page.edit(
            "AccessKey ID", null, "", InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS,
            value -> AliyunTranslationPolicy.issue(config, "access_key_id", value), "aliyun_translation", "access_key_id"));
        group.nav("AccessKey Secret", null, filled(config, "access_key_secret"), () -> page.edit(
            "AccessKey Secret", null, "", InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD,
            value -> AliyunTranslationPolicy.issue(config, "access_key_secret", value), "aliyun_translation", "access_key_secret"));
        choice(page, group, config, catalog.optJSONArray("regions"), "region", "地域");
        choice(page, group, config, catalog.optJSONArray("editions"), "edition", "版本");
        if ("professional".equals(AliyunTranslationPolicy.text(config, "edition")))
            choice(page, group, config, catalog.optJSONArray("scenes"), "scene", "专业场景");
        group.help("使用阿里云机器翻译 AccessKey，保存在本机，不随设置同步。建议使用只授予机器翻译权限的 RAM 子账号。通用版与专业版需分别开通，按所选地域、版本和场景调用并计费。");
    }

    private static String filled(JSONObject config, String key) {
        return AliyunTranslationPolicy.text(config, key).isEmpty() ? "未填写" : "已填写";
    }

    private static void choice(TranslationServicePage page, GroupCard group, JSONObject config,
            JSONArray options, String key, String title) {
        String current = AliyunTranslationPolicy.text(config, key);
        String label = current;
        if (options == null) return;
        for (int index = 0; index < options.length(); index++) {
            JSONArray option = options.optJSONArray(index);
            if (option != null && current.equals(option.optString(0))) label = option.optString(1);
        }
        group.nav(title, null, label, () -> {
            OptionSheet sheet = new OptionSheet(page.requireContext(), title, null);
            for (int index = 0; index < options.length(); index++) {
                JSONArray option = options.optJSONArray(index);
                if (option == null) continue;
                String id = JsonPolicy.strictStringOrEmpty(option.opt(0));
                String name = JsonPolicy.strictStringOrEmpty(option.opt(1));
                sheet.option(name, current.equals(id), () -> KeyboardSheets.save(page,
                    preferences -> KeyboardSheets.child(preferences, "aliyun_translation").put(key, id),
                    page::reload, page::reload));
            }
            sheet.show();
        });
    }
}
