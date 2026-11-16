import app.msime.android.CandidateGlossCardModel;
import app.msime.android.CandidateGlossCardModel.Source;
import app.msime.android.CandidateTranslationRoute;
import java.util.List;
import java.util.Map;

/** 长按候选打开的释义卡片（#10915）：分段、来源名、两个按钮作用于哪一项、暂无译文时的说明。 */
public final class CandidateGlossCardModelSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 离线释义有多个义项时逐条列出，两个按钮都作用于第一项。
        CandidateGlossCardModel offline = CandidateGlossCardModel.of("面试", List.of("en"),
            Map.of("en", "interview; audition"), Map.of(), Source.ACCOUNT);
        check(offline.sections().size() == 1, "one target language gives one section");
        check("英语 · 离线释义".equals(offline.sections().get(0).label()), "label names language and source");
        check(offline.sections().get(0).senses().equals(List.of("interview", "audition")),
            "offline senses are listed one by one");
        check("interview".equals(offline.primary()), "commit and copy act on the first sense");
        check("interview; audition".equals(offline.translation()), "read aloud and wordbook get the shown translation");
        check(offline.senses().equals(List.of("interview", "audition")), "every listed sense can be inserted");

        // 离线优先，查不到才用在线译文；来源名跟着所选的服务。
        CandidateGlossCardModel preferOffline = CandidateGlossCardModel.of("你好", List.of("en"),
            Map.of("en", "hello"), Map.of("en", "hi there"), Source.TENCENT);
        check(preferOffline.sections().get(0).source() == Source.OFFLINE, "offline wins over online");
        CandidateGlossCardModel onlineOnly = CandidateGlossCardModel.of("水杉", List.of("ja"),
            Map.of(), Map.of("ja", "メタセコイア"), Source.NIUTRANS);
        check("日语 · 小牛翻译".equals(onlineOnly.sections().get(0).label()), "online section names the service");
        check("メタセコイア".equals(onlineOnly.primary()), "online translation is committed");
        check(CandidateGlossCardModel.of("水杉", List.of("ja"), Map.of(), Map.of("ja", "x"), null).empty(),
            "without an online service online rows are not used");

        // 两个目标语言时两段都列，顺序按目标语言；第一段决定按钮。
        CandidateGlossCardModel two = CandidateGlossCardModel.of("你好", List.of("ja", "en"),
            Map.of("en", "hello"), Map.of("ja", "こんにちは"), Source.CUSTOM);
        check(two.sections().size() == 2, "two targets give two sections");
        check("日语 · 自定义服务".equals(two.sections().get(0).label())
            && "英语 · 离线释义".equals(two.sections().get(1).label()), "sections follow target order");
        check("こんにちは".equals(two.primary()), "the first target language is committed");

        // 目标语言不是英语、又没有那种语言的译文时，候选栏画的是英文释义，卡片补上英语那一段。
        CandidateGlossCardModel fallback = CandidateGlossCardModel.of("你好", List.of("fr"),
            Map.of("en", "hello"), Map.of(), null);
        check("英语 · 离线释义".equals(fallback.sections().get(0).label()), "English gloss fills an empty card");
        // 第一目标语言没有译文时两个按钮不可用，不拿别的语言顶上，读屏说明原因。
        check(fallback.primary().isEmpty(), "the English fallback is shown but not committed for a French target");
        check("，暂无法语译文".equals(fallback.unavailableReason()), "disabled buttons explain why");
        CandidateGlossCardModel secondOnly = CandidateGlossCardModel.of("你好", List.of("ja", "en"),
            Map.of("en", "hello"), Map.of(), Source.CUSTOM);
        check(secondOnly.sections().size() == 1 && secondOnly.primary().isEmpty(),
            "a second language never stands in for the first");
        check(offline.unavailableReason().isEmpty(), "available buttons need no reason");
        CandidateGlossCardModel noFallback = CandidateGlossCardModel.of("你好", List.of("fr"),
            Map.of("en", "hello"), Map.of("fr", "bonjour"), Source.AI);
        check(noFallback.sections().size() == 1 && "法语 · AI 辅助".equals(noFallback.sections().get(0).label()),
            "the English fallback only appears when nothing else does");

        // 暂无译文：没有服务时说明去设置里开，请求中说正在请求，请求过了不再多说。
        CandidateGlossCardModel empty = CandidateGlossCardModel.of("水杉", List.of("en"), Map.of(), Map.of(), null);
        check(empty.empty() && empty.primary().isEmpty() && empty.translation().isEmpty(), "empty card has nothing to commit");
        check("可以在设置里开启翻译服务".equals(CandidateGlossCardModel.emptyHint(false, false)), "hint without a service");
        check("正在请求译文…".equals(CandidateGlossCardModel.emptyHint(true, true)), "hint while requesting");
        check(CandidateGlossCardModel.emptyHint(true, false).isEmpty(), "no hint after an empty answer");
        check(CandidateGlossCardModel.of("水杉", List.of("en"), Map.of("en", " ; "), Map.of(), null).empty(),
            "blank senses do not make a section");

        // 只为离线和在线都没有的语言单独请求。
        check(CandidateGlossCardModel.missing(List.of("en"), Map.of("en", "hello"), Map.of()).isEmpty(),
            "an offline gloss needs no request");
        check(CandidateGlossCardModel.missing(List.of("ja"), Map.of("en", "hello"), Map.of()).equals(List.of("ja")),
            "a target without a translation is requested");
        check(CandidateGlossCardModel.missing(List.of("ja"), Map.of(), Map.of("ja", "x")).isEmpty(),
            "a cached online translation needs no request");

        // 来源名与各端一致。
        check(Source.online(CandidateTranslationRoute.Provider.ACCOUNT) == Source.ACCOUNT
            && Source.online(CandidateTranslationRoute.Provider.TENCENT) == Source.TENCENT
            && Source.online(CandidateTranslationRoute.Provider.NIUTRANS) == Source.NIUTRANS
            && Source.online(CandidateTranslationRoute.Provider.CUSTOM) == Source.CUSTOM
            && Source.online(CandidateTranslationRoute.Provider.AI) == Source.AI
            && Source.online(CandidateTranslationRoute.Provider.ALIYUN) == Source.ALIYUN
            && Source.online(CandidateTranslationRoute.Provider.NONE) == null, "providers map to source names");
        String[] titles = {"离线释义", "水杉账号", "腾讯翻译", "小牛翻译", "自定义服务", "AI 辅助", "阿里云翻译"};
        for (int index = 0; index < titles.length; index++)
            check(titles[index].equals(Source.values()[index].title()), "source title " + titles[index]);
        check("释义".equals(CandidateGlossCardModel.TITLE) && "暂无译文".equals(CandidateGlossCardModel.EMPTY)
            && "上屏译文".equals(CandidateGlossCardModel.COMMIT) && "复制译文".equals(CandidateGlossCardModel.COPY),
            "card texts match the issue");
        check("西班牙语".equals(CandidateGlossCardModel.languageName("es"))
            && "xx".equals(CandidateGlossCardModel.languageName("xx")), "language names");
        System.out.println("Android candidate gloss card: sections, sources, buttons and empty hint passed");
    }
}
