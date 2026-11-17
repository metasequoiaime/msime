package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** 只聚合共享层已分类的每日按键；不在 Java 猜测来源或补写旧历史。 */
public final class TypingKeyLanguages {
    public enum Language {
        CHINESE("chinese", "中文"), ENGLISH("english", "英文"),
        JAPANESE("japanese", "日文"), KOREAN("korean", "韩文"),
        CANTONESE("cantonese", "粤语"), VIETNAMESE("vietnamese", "越南文"),
        TIBETAN("tibetan", "藏文"), COMMON("common", "通用键"), UNKNOWN("unknown", "未分类");

        private final String id;
        private final String label;

        Language(String id, String label) { this.id = id; this.label = label; }
        public String id() { return id; }
        public String label() { return label; }
    }

    private final Map<String, Map<String, Map<String, Long>>> days;

    public TypingKeyLanguages(Map<String, Map<String, Map<String, Long>>> days) {
        Map<String, Map<String, Map<String, Long>>> copy = new LinkedHashMap<>();
        days.forEach((day, languages) -> {
            Map<String, Map<String, Long>> groups = new LinkedHashMap<>();
            languages.forEach((language, keys) -> groups.put(language, Map.copyOf(keys)));
            copy.put(day, Map.copyOf(groups));
        });
        this.days = Map.copyOf(copy);
    }

    public Map<String, Long> keys(Iterable<String> selectedDays, Language language) {
        if (selectedDays == null || language == null) return Map.of();
        Map<String, Long> result = new LinkedHashMap<>();
        for (String day : selectedDays) {
            Map<String, Long> keys = days.getOrDefault(day, Map.of())
                .getOrDefault(language.id(), Map.of());
            keys.forEach((key, count) -> {
                if (count > 0) result.merge(key, count,
                    (left, right) -> left > Long.MAX_VALUE - right ? Long.MAX_VALUE : left + right);
            });
        }
        return Map.copyOf(result);
    }

    /** 只列窗口内有记录的分组；旧共享库缺少语言字段时仅保留页面的「全部」。 */
    public List<Language> available(List<String> selectedDays) {
        List<Language> result = new ArrayList<>();
        for (Language language : Language.values())
            if (!keys(selectedDays, language).isEmpty()) result.add(language);
        return List.copyOf(result);
    }
}
