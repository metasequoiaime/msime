package app.msime.android;

// 由共享 Rust 语音语言目录导出；通过 export_voice_language_catalog 更新。
import java.util.ArrayList;
import java.util.List;

public final class VoiceLanguageCatalog {
    public record Language(String id, String value, String title) {}

    private static final Language[] ALL = {
        new Language("zh-cn", "zh-cn", "普通话"),
        new Language("yue", "yue", "粤语"),
        new Language("en", "en", "英语"),
        new Language("ja", "", "日语"),
        new Language("auto", "auto", "普通话 + 英语"),
    };

    private VoiceLanguageCatalog() {}

    public static List<Language> mobile() {
        var result = new ArrayList<Language>();
        for (Language language : ALL) {
            if (!language.value().isEmpty()) result.add(language);
        }
        return List.copyOf(result);
    }

    public static String defaultValue() {
        return ALL[0].value();
    }

    public static String title(String value) {
        if (value == null || value.isEmpty()) return ALL[0].title();
        for (Language language : ALL) {
            if (language.id().equals(value)) return language.title();
        }
        return value;
    }
}
