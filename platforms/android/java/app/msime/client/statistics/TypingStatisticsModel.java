package app.msime.client;

import java.time.LocalDate;
import java.time.format.DateTimeParseException;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 共享打字统计文档的只读视图。
 *
 * <p>The store keeps aggregate counts and nothing else -- dates, character classes, commit sources
 * and totals, never the text that produced them -- and this reader keeps that boundary. What it adds
 * is the arithmetic the statistics page needs: a dense trend series from a sparse day map, and the
 * three distributions the page groups those counts into.
 *
 * <p>A document this cannot read is reported as absent rather than as zero. "Nothing recorded" and
 * "no characters typed" say different things to whoever is reading the page.
 */
public final class TypingStatisticsModel {
    /** 趋势窗口最多画一年；这是图表的宽度上限，不是保留期限，更早的每日明细仍在存储里。 */
    public static final int MAX_TREND_DAYS = 366;

    /** Which distribution the page is showing. Trend is the series rather than a distribution. */
    public enum Section {
        TREND("趋势", "每日趋势"),
        KIND("类型", "字符类型"),
        MODE("模式", "语言模式"),
        SCHEME("方案", "输入方案");

        private final String tab;
        private final String heading;

        Section(String tab, String heading) {
            this.tab = tab;
            this.heading = heading;
        }

        /** 分段控件上只放两个字，全名留给下面的分组标题。 */
        public String tab() { return tab; }

        public String heading() { return heading; }
    }

    /** One row of a distribution: a stable id, what to call it, and how many characters. */
    public record Slice(String id, String title, long count) {}

    private static final Map<String, String> CHARACTER_KINDS = kinds();
    private static final Map<String, String> SOURCES = sources();
    private static final List<String> CHINESE_SOURCES = List.of(
        "quanpin", "nineKey", "shuangpin", "ziranma", "microsoft", "shoudao", "wubi");

    private final boolean enabled;
    private final long total;
    private final String retention;
    private final Map<String, Long> days;
    private final Map<String, Long> characters;
    private final Map<String, Long> commitSources;
    private final Map<String, Map<String, Long>> dailyCharacters;
    private final Map<String, Map<String, Long>> dailySources;

    /**
     * @param days characters per `YYYY-MM-DD` day
     * @param characters lifetime counts per character class
     * @param commitSources lifetime counts per commit source
     * @param dailyCharacters per-day character classes, for the days that have them
     * @param dailySources per-day commit sources, for the days that have them
     */
    public TypingStatisticsModel(boolean enabled, long total, String retention,
            Map<String, Long> days, Map<String, Long> characters, Map<String, Long> commitSources,
            Map<String, Map<String, Long>> dailyCharacters,
            Map<String, Map<String, Long>> dailySources) {
        this.enabled = enabled;
        this.total = total;
        this.retention = retention;
        this.days = days;
        this.characters = characters;
        this.commitSources = commitSources;
        this.dailyCharacters = dailyCharacters;
        this.dailySources = dailySources;
    }

    public boolean enabled() { return enabled; }

    public long total() { return total; }

    /** `forever`, `30d`, `90d`, `180d` or `365d`. */
    public String retention() { return retention; }

    /** Characters recorded on one `YYYY-MM-DD` day. */
    public long count(String day) {
        Long value = days.get(day);
        return value == null ? 0 : value;
    }

    /** The most recent day that has a record, or an empty string when nothing was ever recorded. */
    public String latestDay() {
        String latest = "";
        for (String day : days.keySet()) {
            if (day.compareTo(latest) > 0) latest = day;
        }
        return latest;
    }

    /** How many days back a trend can usefully run: the earliest record, capped at a year. */
    public int recordedSpan(String today) {
        String earliest = "";
        for (String day : days.keySet()) {
            if (earliest.isEmpty() || day.compareTo(earliest) < 0) earliest = day;
        }
        if (earliest.isEmpty()) return 0;
        LocalDate start = day(earliest);
        LocalDate end = day(today);
        if (start == null || end == null || start.isAfter(end)) return 0;
        long span = end.toEpochDay() - start.toEpochDay() + 1;
        return KeyboardGeometry.bounded((int) Math.min(MAX_TREND_DAYS, span), 1, MAX_TREND_DAYS);
    }

    /**
     * A dense series of `length` daily counts ending on `today`.
     *
     * <p>Dense because the chart draws a point per day: a sparse map would let a week of silence
     * render as a straight line between the two days either side of it.
     */
    public int[] trend(String today, int length) {
        if (length <= 0) return new int[0];
        int bounded = KeyboardGeometry.bounded(length, 0, MAX_TREND_DAYS);
        LocalDate end = day(today);
        int[] series = new int[bounded];
        if (end == null) return series;
        for (int index = 0; index < bounded; index++) {
            LocalDate date = end.minusDays(bounded - 1L - index);
            series[index] = (int) Math.min(Integer.MAX_VALUE, count(date.toString()));
        }
        return series;
    }

    /** The day key each cell of {@link #trend} was taken from, in the same order. */
    public List<String> trendDays(String today, int length) {
        int bounded = KeyboardGeometry.bounded(length, 0, MAX_TREND_DAYS);
        LocalDate end = day(today);
        if (end == null || bounded == 0) return List.of();
        List<String> result = new ArrayList<>(bounded);
        for (int index = 0; index < bounded; index++) {
            result.add(end.minusDays(bounded - 1L - index).toString());
        }
        return List.copyOf(result);
    }

    /**
     * One distribution, ordered and titled, for the whole record or for a single day.
     *
     * @param day a `YYYY-MM-DD` key to scope to, or {@code null} for the running total
     */
    public List<Slice> slices(Section section, String day) {
        if (section == Section.TREND) return List.of();
        long scope = day == null ? total : count(day);
        if (section == Section.KIND) {
            Map<String, Long> values = unclassified(
                day == null ? characters : dailyCharacters.getOrDefault(day, Map.of()), scope);
            return named(CHARACTER_KINDS, values);
        }
        Map<String, Long> values = unclassified(
            day == null ? commitSources : dailySources.getOrDefault(day, Map.of()), scope);
        if (section == Section.SCHEME) return named(SOURCES, values);
        return modes(values);
    }

    /** The sum of a distribution, which is the scope's character count including the unclassified. */
    public static long sum(List<Slice> slices) {
        long total = 0;
        for (Slice slice : slices) total += slice.count();
        return total;
    }

    /**
     * 语言模式：按提交时用的键盘模式归并，不推测文本语言。
     *
     * <p>Handwriting is its own mode here. The character it commits came from a stroke rather than
     * from any of the pinyin schemes, and folding it into 中文模式 would make the modes disagree
     * with the scheme list they are grouping.
     */
    private static List<Slice> modes(Map<String, Long> values) {
        long chinese = 0;
        for (String id : CHINESE_SOURCES) chinese += values.getOrDefault(id, 0L);
        List<Slice> slices = new ArrayList<>();
        slices.add(new Slice("chinese", "中文模式", chinese));
        slices.add(new Slice("japanese", "日语模式", values.getOrDefault("japanese", 0L)));
        slices.add(new Slice("english", "英文模式", values.getOrDefault("english", 0L)));
        slices.add(new Slice("handwriting", "手写输入", values.getOrDefault("handwriting", 0L)));
        slices.add(new Slice("local", "本地输入", values.getOrDefault("local", 0L)));
        slices.add(new Slice("ai", "AI 润色", values.getOrDefault("ai", 0L)));
        slices.add(new Slice("reply", "高情商回复", values.getOrDefault("reply", 0L)));
        slices.add(new Slice("voice", "语音输入", values.getOrDefault("voice", 0L)));
        slices.add(new Slice("unknown", "历史未分类", values.getOrDefault("unknown", 0L)));
        return List.copyOf(slices);
    }

    private static List<Slice> named(Map<String, String> titles, Map<String, Long> values) {
        List<Slice> slices = new ArrayList<>(titles.size());
        for (Map.Entry<String, String> entry : titles.entrySet()) {
            slices.add(new Slice(entry.getKey(), entry.getValue(),
                values.getOrDefault(entry.getKey(), 0L)));
        }
        return List.copyOf(slices);
    }

    /**
     * Counts with the shortfall against the scope's total filed as unclassified.
     *
     * <p>Versions before the breakdown existed recorded a day's characters without classifying
     * them. Dropping the difference would make a pie chart of one profile's history disagree with
     * the total printed above it.
     */
    private static Map<String, Long> unclassified(Map<String, Long> values, long scope) {
        long classified = 0;
        for (long count : values.values()) classified += count;
        if (scope <= classified) return values;
        Map<String, Long> result = new LinkedHashMap<>(values);
        result.merge("unknown", scope - classified, Long::sum);
        return result;
    }

    private static LocalDate day(String value) {
        if (value == null || value.isEmpty()) return null;
        try {
            return LocalDate.parse(value);
        } catch (DateTimeParseException error) {
            return null;
        }
    }

    private static Map<String, String> kinds() {
        Map<String, String> titles = new LinkedHashMap<>();
        titles.put("han", "汉字");
        titles.put("latin", "拉丁字母");
        titles.put("otherLetter", "其他文字");
        titles.put("number", "数字");
        titles.put("punctuation", "标点");
        titles.put("emoji", "表情");
        titles.put("symbol", "其他符号");
        titles.put("unknown", "历史未分类");
        // Order carries meaning here, so it stays a LinkedHashMap; Map.copyOf would drop it.
        return java.util.Collections.unmodifiableMap(titles);
    }

    private static Map<String, String> sources() {
        Map<String, String> titles = new LinkedHashMap<>();
        titles.put("quanpin", "全拼 26 键");
        titles.put("nineKey", "全拼 9 键");
        titles.put("shuangpin", "小鹤双拼");
        titles.put("ziranma", "自然码双拼");
        titles.put("microsoft", "微软双拼");
        titles.put("shoudao", "首道双拼");
        titles.put("wubi", "86 五笔");
        titles.put("japanese", "日语");
        titles.put("handwriting", "手写");
        titles.put("english", "英文键盘");
        titles.put("local", "本地输入");
        titles.put("ai", "AI 润色");
        titles.put("reply", "高情商回复");
        titles.put("voice", "语音输入");
        titles.put("unknown", "历史未分类");
        return java.util.Collections.unmodifiableMap(titles);
    }
}
