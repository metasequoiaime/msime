package app.msime.android.test;

import android.os.Bundle;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.time.LocalDate;
import java.util.function.Predicate;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在原生键盘上输入日期触发词，展开候选并选中生成日期，核对编辑器只收到结果。 */
public final class RelativeDateDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "全拼与九键的相对日期、年份候选和整段上屏";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        android.content.SharedPreferences modes = getTargetContext()
            .getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        try {
            for (boolean nineKey : new boolean[] {false, true}) {
                JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin")
                    .put("touch_keyboard_layout", nineKey ? "nine_key" : "twenty_six_key")
                    .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
                    .put("cloud_candidates", false).put("learning", false)
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("touch_keyboard_schemes", new JSONObject()
                        .put("enabled", new JSONArray().put("quanpin").put("nine_key"))
                        .put("selected", nineKey ? "nine_key" : "quanpin"));
                // 局部模式含必填字段；已有对象就保留其余字段，缺省对象沿用出厂开启值。
                JSONObject localModes = base.optJSONObject("local_modes");
                if (localModes != null) localModes.put("date_time_inline", true).put("date_time_menu", true);
                publish(preferences, new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", base).toString()
                    .getBytes(StandardCharsets.UTF_8));
                if (!modes.edit().clear().commit()) throw new AssertionError("输入模式准备失败");
                restartIme();
                // 不等待启动命令进入空闲；以编辑器和键盘节点同步，避免 IME 重绑等待环。
                shellDiscard("am start -f 0x10008000 -n app.msime.android.test/.EditorActivity");
                tap(field("msime-test-plain"));
                if (equalsText("英", await(key("中").or(key("英"))).getText())) tap(key("英"));
                String committed = "";
                for (String[] entry : new String[][] {
                    {"riqi", "今天", "0"}, {"jintian", "今天", "0"},
                    {"zuotian", "昨天", "-1"}, {"qiantian", "前天", "-2"},
                    {"mingtian", "明天", "1"}, {"houtian", "后天", "2"},
                    {"qunian", "去年", "-1y"}, {"jinnian", "今年", "0y"},
                    {"mingnian", "明年", "1y"}, {"lainian", "来年", "1y"}}) {
                    String scenario = (nineKey ? "九键 " : "全拼 ") + entry[0];
                    stage = scenario + " 输入";
                    LocalDate today = LocalDate.now();
                    String expected;
                    if (entry[2].endsWith("y")) {
                        int offset = Integer.parseInt(entry[2].substring(0, entry[2].length() - 1));
                        expected = entry[1] + "(" + (today.getYear() + offset) + "年)";
                    } else {
                        LocalDate date = today.plusDays(Integer.parseInt(entry[2]));
                        expected = entry[1] + "(" + date.getMonthValue() + "月" + date.getDayOfMonth() + "日)";
                    }
                    typeKeyword(entry[0], nineKey);
                    stage = scenario + " 展开候选";
                    tap(key("展开").and(AccessibilityNodeInfo::isEnabled));
                    stage = scenario + " 选择日期";
                    tap(dateCandidate(expected));
                    committed += expected;
                    stage = scenario + " 整段上屏";
                    String result = committed;
                    await(field("msime-test-plain").and(node -> equalsText(result, node.getText())));
                    if (!LocalDate.now().equals(today)) throw new AssertionError("验收跨过午夜，请重跑");
                }
                Bundle clear = new Bundle();
                clear.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "");
                if (!await(field("msime-test-plain")).performAction(
                        AccessibilityNodeInfo.ACTION_SET_TEXT, clear))
                    throw new AssertionError("合成编辑器清空失败");
            }
        } finally {
            shellDiscard("am start -n app.msime.android/.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            android.content.SharedPreferences.Editor restore = modes.edit().clear();
            for (java.util.Map.Entry<String, ?> entry : originalModes.entrySet())
                restore.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!restore.commit()) throw new AssertionError("输入模式恢复失败");
            restartIme();
        }
    }

    private void typeKeyword(String keyword, boolean nineKey) throws Exception {
        String[] keys = {"ABC", "DEF", "GHI", "JKL", "MNO", "PQRS", "TUV", "WXYZ"};
        for (char letter : keyword.toCharArray()) {
            if (!nineKey) tap(described("字母 " + Character.toUpperCase(letter)));
            else {
                int digit = letter <= 'c' ? 2 : letter <= 'f' ? 3 : letter <= 'i' ? 4
                    : letter <= 'l' ? 5 : letter <= 'o' ? 6 : letter <= 's' ? 7 : letter <= 'v' ? 8 : 9;
                tap(key(keys[digit - 2]));
            }
        }
    }

    private Predicate<AccessibilityNodeInfo> dateCandidate(String text) {
        Pattern description = Pattern.compile("候选 [0-9]+：" + Pattern.quote(text) + "(?:；.*)?");
        return node -> node.isClickable() && node.getContentDescription() != null
            && description.matcher(node.getContentDescription()).matches();
    }
}
