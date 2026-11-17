import app.msime.android.TypingKeyLanguages;
import app.msime.android.TypingKeyLanguages.Language;
import app.msime.android.TypingStatisticsModel;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** 语言筛选只使用共享层的每日聚合，功能键与未知历史不混进语言计数。 */
public final class TypingKeyLanguagesSmoke {
    public static void main(String[] args) {
        Map<String, Long> chinese = new HashMap<>(Map.of("KeyA", 2L));
        Map<String, Map<String, Long>> today = new HashMap<>(Map.of(
            "chinese", chinese, "english", Map.of("KeyA", 3L),
            "common", Map.of("ShiftLeft", 4L), "unknown", Map.of("KeyA", 5L)));
        TypingKeyLanguages languages = new TypingKeyLanguages(Map.of(
            "2026-11-17", today, "2026-10-01", Map.of("chinese", Map.of("KeyA", 20L))));
        chinese.put("KeyA", 99L);
        today.clear();
        List<String> days = List.of("2026-11-16", "2026-11-17");
        check(languages.keys(days, Language.CHINESE).equals(Map.of("KeyA", 2L)),
            "窗口外旧日不混入，快照不跟随调用方修改");
        check(languages.keys(days, Language.COMMON).equals(Map.of("ShiftLeft", 4L)),
            "功能键独立筛选");
        check(languages.keys(days, Language.UNKNOWN).equals(Map.of("KeyA", 5L)),
            "旧历史的未分类不冒充当前语言");
        check(languages.available(days).equals(List.of(
            Language.CHINESE, Language.ENGLISH, Language.COMMON, Language.UNKNOWN)),
            "只列窗口内有数据的有限语言");
        TypingStatisticsModel model = new TypingStatisticsModel(true, 0, "forever",
            Map.of(), Map.of(), Map.of(), Map.of(), Map.of(),
            Map.of("2026-11-17", Map.of("KeyA", 10L, "ShiftLeft", 4L)), languages);
        check(model.keysForDays(days, null).equals(Map.of("KeyA", 10L, "ShiftLeft", 4L)),
            "全部仍使用原总计，不漏掉旧数据");
        check(model.keysForDays(days, Language.ENGLISH).equals(Map.of("KeyA", 3L)),
            "页面可读到所选语言的计数");
        check(new TypingKeyLanguages(Map.of()).available(days).isEmpty(),
            "旧共享库缺字段时不生成假的语言分类");
        System.out.println("Android 按键语言筛选保留日期窗口、通用键与未知历史");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
