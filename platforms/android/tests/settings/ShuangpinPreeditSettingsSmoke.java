import app.msime.android.AppEdition;
import app.msime.android.ShuangpinPreeditSettings;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/** 运行键盘页共用的设置行构建，验证版本可达性、缺字段默认值及开关回调的保存目标。 */
public final class ShuangpinPreeditSettingsSmoke {
    private record Row(String title, boolean checked, Consumer<Boolean> change, String help) {}

    public static void main(String[] args) {
        Map<String, Boolean> preferences = new HashMap<>();
        List<Row> rows = render(AppEdition.FULL, preferences);
        check(rows.size() == 2, "完整版列出两个开关");
        check(rows.get(0).checked() && !rows.get(1).checked(), "缺字段仍默认原始按键开、分隔关");
        check(rows.get(0).title().equals("双拼显示原始按键")
            && rows.get(1).title().equals("原始按键按音节分隔"), "两行标题对应现有设置");
        check(rows.stream().allMatch(row -> !row.help().isEmpty()), "说明交给标题旁的帮助组件");
        check(preferences.isEmpty(), "绘制设置不写入默认值");
        rows.get(1).change().accept(true);
        check(preferences.equals(Map.of("shuangpin_preedit_separated", true)), "分隔开关只保存分隔字段");
        rows.get(0).change().accept(false);
        check(preferences.equals(Map.of("shuangpin_preedit_uses_raw", false,
            "shuangpin_preedit_separated", true)), "原始按键开关保留之前的分隔选择");
        rows = render(AppEdition.FULL, preferences);
        check(rows.size() == 1 && !rows.get(0).checked(), "关闭原始按键后隐藏分隔行");
        rows.get(0).change().accept(true);
        rows = render(AppEdition.FULL, preferences);
        check(rows.size() == 2 && rows.get(1).checked(), "重新开启后读回保留的分隔值");

        for (String scheme : List.of("quanpin", "shuangpin", "cantonese", "wubi", "japanese")) {
            rows = render(AppEdition.of("synthetic-" + scheme, scheme, scheme, false), new HashMap<>());
            int expected = scheme.equals("shuangpin") ? 2 : scheme.equals("cantonese") ? 1 : 0;
            check(rows.size() == expected, scheme + " 版本只列出能够生效的设置");
        }
        System.out.println("ShuangpinPreeditSettingsSmoke ok");
    }

    private static List<Row> render(AppEdition edition, Map<String, Boolean> preferences) {
        List<Row> rows = new ArrayList<>();
        ShuangpinPreeditSettings.addRows(edition, preferences::getOrDefault, preferences::put,
            (title, checked, change, help) -> rows.add(new Row(title, checked, change, help)));
        return rows;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
