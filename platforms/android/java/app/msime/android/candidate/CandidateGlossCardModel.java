package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 长按候选打开的「释义」卡片（#10915）里显示什么：每个目标语言一段（触屏键盘只有主目标语言，见 {@link CandidateTranslationPolicy#MAX_TARGETS}；有几种就列几段），段首一行「目标语言名 · 来源名」，下面逐条列出义项；底部「上屏译文」「复制译文」作用于第一目标语言那一段的第一个义项，第一目标语言没有译文时两个按钮都不可用，不拿别的语言顶上。iOS、HarmonyOS 用同一套文字和规则。
 *
 * <p>只用现有的候选翻译通路：离线释义词典和用户已经选好的在线翻译服务。某个目标语言离线释义查得到就用离线的，查不到才用在线译文，与候选栏的 {@link CandidateTranslationPolicy#mergeGlosses} 相同。没有一段可显示、而离线英文释义有的时候，补一段英语：目标语言不是英语、又没装那种语言的离线词典时，候选栏上画的就是英文释义，卡片不能比候选栏少。
 *
 * <p>参数都是基本类型和集合，JVM 冒烟加载不了 org.json。
 */
public final class CandidateGlossCardModel {
    public static final String TITLE = "释义";
    public static final String EMPTY = "暂无译文";
    public static final String EMPTY_HINT = "可以在设置里开启翻译服务";
    public static final String PENDING_HINT = "正在请求译文…";
    public static final String COMMIT = "上屏译文";
    public static final String COPY = "复制译文";
    public static final String CLOSE = "收起";
    public static final String CLOSE_DESCRIPTION = "收起释义";
    /** 盖在候选栏和读音行上的那一层：点上去只关卡片，不选词。 */
    public static final String SHIELD_DESCRIPTION = "关闭释义";
    public static final String COPIED = "已复制译文";
    public static final String COPY_FAILED = "无法复制译文";
    public static final String STALE = "候选已更新，请重新长按";
    /** 候选读屏描述末尾的长按提示，替换原来的「；长按管理」。 */
    public static final String LONG_PRESS_HINT = "；长按查看释义";

    private static final Map<String, String> LANGUAGE_NAMES = Map.of(
        "en", "英语", "fr", "法语", "ja", "日语", "es", "西班牙语", "ru", "俄语", "de", "德语", "ko", "韩语");

    /** 译文从哪里来；名字和 iOS、HarmonyOS 的卡片相同。 */
    public enum Source {
        OFFLINE("离线释义"),
        ACCOUNT("水杉账号"),
        TENCENT("腾讯翻译"),
        NIUTRANS("小牛翻译"),
        CUSTOM("自定义服务"),
        AI("AI 辅助");

        private final String title;

        Source(String title) { this.title = title; }

        public String title() { return title; }

        /** 在线服务的来源名；不联网时为 null。 */
        public static Source online(CandidateTranslationRoute.Provider provider) {
            if (provider == null) return null;
            return switch (provider) {
                case ACCOUNT -> ACCOUNT;
                case TENCENT -> TENCENT;
                case NIUTRANS -> NIUTRANS;
                case CUSTOM -> CUSTOM;
                case AI -> AI;
                case NONE -> null;
            };
        }
    }

    /** 一个目标语言的一段：{@code text} 是原样的译文，{@code senses} 是切好的义项（{@link CandidateTranslationPolicy#insertionGlosses}），至少一项。 */
    public record Section(String language, Source source, String text, List<String> senses) {
        public Section {
            if (language == null || source == null || text == null || senses == null || senses.isEmpty())
                throw new IllegalArgumentException("Invalid gloss card section");
            senses = List.copyOf(senses);
        }

        /** 段首一行：目标语言名 · 来源名。 */
        public String label() { return languageName(language) + " · " + source.title(); }
    }

    private final String candidate;
    private final String firstTarget;
    private final List<Section> sections;

    private CandidateGlossCardModel(String candidate, String firstTarget, List<Section> sections) {
        this.candidate = candidate;
        this.firstTarget = firstTarget;
        this.sections = List.copyOf(sections);
    }

    /**
     * 按目标语言顺序拼出卡片：每种语言先取 {@code offline}，没有再取 {@code online}，两者都没有就不出这一段。{@code offline} 里的英语是随包的英文释义；{@code onlineSource} 为 null 时不用 {@code online}。
     */
    public static CandidateGlossCardModel of(String candidate, List<String> targets, Map<String, String> offline,
                                             Map<String, String> online, Source onlineSource) {
        String text = candidate == null ? "" : candidate;
        ArrayList<Section> sections = new ArrayList<>(2);
        if (targets != null) {
            for (String target : targets) {
                Section section = section(target, Source.OFFLINE, offline);
                if (section == null && onlineSource != null) section = section(target, onlineSource, online);
                if (section != null) sections.add(section);
            }
        }
        if (sections.isEmpty() && (targets == null || !targets.contains("en"))) {
            Section english = section("en", Source.OFFLINE, offline);
            if (english != null) sections.add(english);
        }
        String first = targets == null || targets.isEmpty() ? "" : targets.get(0);
        return new CandidateGlossCardModel(text, first, sections);
    }

    private static Section section(String language, Source source, Map<String, String> values) {
        String value = values == null ? null : values.get(language);
        List<String> senses = CandidateTranslationPolicy.insertionGlosses(value);
        return senses.isEmpty() ? null : new Section(language, source, value, senses);
    }

    public String candidate() { return candidate; }

    public List<Section> sections() { return sections; }

    public boolean empty() { return sections.isEmpty(); }

    /** 「上屏译文」和「复制译文」的内容：第一目标语言那一段的第一个义项；第一目标语言没有译文时为空串，两个按钮不可用。 */
    public String primary() {
        for (Section section : sections)
            if (section.language().equals(firstTarget)) return section.senses().get(0);
        return "";
    }

    /** 两个按钮不可用时补给读屏的原因；可用时为空串。 */
    public String unavailableReason() {
        return primary().isEmpty() ? "，暂无" + languageName(firstTarget) + "译文" : "";
    }

    /** 交给「朗读」和「加入生词本」的译文：各段原文按行连起来，与候选栏上的写法（{@link CandidateTranslationPolicy#joinGlosses}）相同。 */
    public String translation() {
        ArrayList<String> rows = new ArrayList<>(sections.size());
        for (Section section : sections) rows.add(section.text());
        return CandidateTranslationPolicy.joinGlosses(rows);
    }

    /** 每一个可以点了上屏的义项，按段落顺序，去掉重复。 */
    public List<String> senses() {
        ArrayList<String> result = new ArrayList<>();
        for (Section section : sections)
            for (String sense : section.senses()) if (!result.contains(sense)) result.add(sense);
        return List.copyOf(result);
    }

    /** 「暂无译文」下面那一句：没有配置可用的在线翻译服务时说明可以去设置里开启，正在请求时说正在请求，请求过了还是没有（或这个候选不送翻译服务）就不再多说。 */
    public static String emptyHint(boolean serviceConfigured, boolean pending) {
        if (!serviceConfigured) return EMPTY_HINT;
        return pending ? PENDING_HINT : "";
    }

    /** 这些目标语言里离线和在线都还没有译文的，按顺序；卡片为它们单独请求一次。 */
    public static List<String> missing(List<String> targets, Map<String, String> offline, Map<String, String> online) {
        if (targets == null) return List.of();
        ArrayList<String> result = new ArrayList<>(targets.size());
        for (String target : targets) {
            if (section(target, Source.OFFLINE, offline) == null && section(target, Source.OFFLINE, online) == null)
                result.add(target);
        }
        return List.copyOf(result);
    }

    /** 目标语言的中文名；不认识的代码原样返回。 */
    public static String languageName(String code) {
        String name = code == null ? null : LANGUAGE_NAMES.get(code);
        return name == null ? String.valueOf(code) : name;
    }
}
