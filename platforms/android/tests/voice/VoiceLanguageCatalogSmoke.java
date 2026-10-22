package app.msime.android;

import java.util.List;

/** 语音语言目录由共享 Rust 导出，Android 只消费移动端有值的投影。 */
public final class VoiceLanguageCatalogSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        List<VoiceLanguageCatalog.Language> languages = VoiceLanguageCatalog.mobile();
        check(languages.stream().map(VoiceLanguageCatalog.Language::value).toList()
                .equals(List.of("zh-cn", "yue", "en", "auto")), "移动端语言顺序和值保持稳定");
        check(languages.stream().map(VoiceLanguageCatalog.Language::title).toList()
                .equals(List.of("普通话", "粤语", "英语", "普通话 + 英语")), "移动端标题来自共享目录");
        check(VoiceLanguageCatalog.defaultValue().equals("zh-cn"), "空值回退到普通话");
        check(VoiceLanguageCatalog.title("").equals("普通话")
                && VoiceLanguageCatalog.title("ja").equals("日语"), "旧值标题保持兼容");
        check(VoiceLanguageCatalog.title("future").equals("future"), "未知值原样显示");
        try {
            languages.clear();
            throw new AssertionError("语言目录不能被界面修改");
        } catch (UnsupportedOperationException expected) {
            check(VoiceLanguageCatalog.mobile().size() == 4, "失败修改不破坏目录");
        }
        System.out.println("Android shared voice language catalog: projection passed");
    }
}
