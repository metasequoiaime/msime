package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 打字统计 `summary` 操作的答复：概览、习惯、按键和徽章四段，以及统计页把这些数字写成文案的规则。
 *
 * <p>指标的口径全部在 Rust（`crates/client-core/src/typing_statistics/metrics.rs`），这里只解析和排版。Rust 给 null 的字段在这里仍是 null（样本不足、没有活跃时间），页面把它们显示成「—」，不当成 0。纯 Java：不引用 androidx、R 或 `home/`，排版规则可以在主机 JVM 上冒烟。
 */
public final class TypingStatisticsSummary {
    /** 一天和这天的字数。 */
    public record DayCount(String day, long count) {}

    /**
     * 概览段。
     *
     * @param averageSpeed 近 7 天每活跃分钟的字数，可空
     * @param previousAverageSpeed 再往前 7 天的同一数值，可空
     * @param firstCandidateRate 首选命中比例（0–1），可空
     * @param keystrokesSavedRate 比全拼少按的比例（0–1），可空
     */
    public record Overview(long weekTotal, long previousWeekTotal, List<DayCount> last7,
            Double averageSpeed, Double previousAverageSpeed, Double firstCandidateRate,
            Double keystrokesSavedRate, long currentStreak, long longestStreak) {}

    /** 计数和最大的连续两小时，`[start, end)`，跨午夜时 `end` 绕回。 */
    public record PeakWindow(int start, int end) {}

    /**
     * 习惯段。
     *
     * @param hours24 近 7 天逐小时字数，恒为 24 项
     * @param peakWindow 高峰时段，可空
     */
    public record Habits(List<DayCount> weeks12, List<Long> hours24, PeakWindow peakWindow,
            long activeDays, Map<String, Long> characters, Map<String, Long> sources) {}

    /** 一次不停顿输入的字数和那天。 */
    public record Run(long characters, String day) {}

    /**
     * 按键段；可空字段的含义同 Rust。
     *
     * @param positions 第 1、2、3 个候选和其余位置的比例，可空
     * @param longestRun 单次最长，可空
     */
    public record Keys(Double perCharacterKeys, Double previousPerCharacterKeys,
            Double backspaceRate, Double predictionRate, Run longestRun, List<Double> positions) {}

    /**
     * 一枚徽章。
     *
     * @param group `volume`、`streak`、`skill` 或 `fun`，决定奖章配色
     * @param unlockedDay 解锁那天，未解锁时为 null
     */
    public record Achievement(String id, String glyph, String title, String description,
            String group, String unlockedDay, long current, long target) {
        public boolean unlocked() { return unlockedDay != null; }

        /** 0–1 的进度；已解锁恒为 1。 */
        public double progress() {
            if (unlocked()) return 1d;
            if (target <= 0) return 0d;
            return BoundsPolicy.bounded((double) current / target, 0d, 1d);
        }
    }

    /** 一段占比：标题和计数。 */
    public record Share(String title, long count) {}

    private final Overview overview;
    private final Habits habits;
    private final Keys keys;
    private final List<Achievement> achievements;

    public TypingStatisticsSummary(Overview overview, Habits habits, Keys keys,
            List<Achievement> achievements) {
        this.overview = overview;
        this.habits = habits;
        this.keys = keys;
        this.achievements = List.copyOf(achievements);
    }

    public Overview overview() { return overview; }

    public Habits habits() { return habits; }

    public Keys keys() { return keys; }

    public List<Achievement> achievements() { return achievements; }

    /** 已解锁的徽章数。 */
    public int unlockedCount() {
        int count = 0;
        for (Achievement achievement : achievements) if (achievement.unlocked()) count++;
        return count;
    }

    /** 解析 `summary` 操作答复的 `value`；缺任何一段时返回 null，页面按「读不到」显示。 */
    public static TypingStatisticsSummary from(JSONObject value) {
        if (value == null) return null;
        JSONObject overview = value.optJSONObject("overview");
        JSONObject habits = value.optJSONObject("habits");
        JSONObject keys = value.optJSONObject("keys");
        JSONArray achievements = value.optJSONArray("achievements");
        if (overview == null || habits == null || keys == null || achievements == null) return null;
        JSONObject breakdown = habits.optJSONObject("breakdown");
        JSONObject peak = habits.optJSONObject("peak_window");
        JSONObject run = keys.optJSONObject("longest_run");
        List<Achievement> badges = new ArrayList<>(achievements.length());
        for (int index = 0; index < achievements.length(); index++) {
            JSONObject badge = achievements.optJSONObject(index);
            if (badge == null) continue;
            badges.add(new Achievement(badge.optString("id", ""), badge.optString("glyph", ""),
                badge.optString("title", ""), badge.optString("description", ""),
                badge.optString("group", "volume"), text(badge, "unlocked_day"),
                count(badge.opt("current")), count(badge.opt("target"))));
        }
        return new TypingStatisticsSummary(
            new Overview(count(overview.opt("week_total")), count(overview.opt("previous_week_total")),
                days(overview.optJSONArray("last7")), number(overview, "average_speed"),
                number(overview, "previous_average_speed"), number(overview, "first_candidate_rate"),
                number(overview, "keystrokes_saved_rate"), count(overview.opt("current_streak")),
                count(overview.opt("longest_streak"))),
            new Habits(days(habits.optJSONArray("weeks12")), hours(habits.optJSONArray("hours24")),
                peak == null ? null : new PeakWindow((int) count(peak.opt("start")),
                    (int) count(peak.opt("end"))),
                count(habits.opt("active_days")),
                breakdown == null ? Map.of() : counts(breakdown.optJSONObject("characters")),
                breakdown == null ? Map.of() : counts(breakdown.optJSONObject("sources"))),
            new Keys(number(keys, "per_character_keys"), number(keys, "previous_per_character_keys"),
                number(keys, "backspace_rate"), number(keys, "prediction_rate"),
                run == null ? null : new Run(count(run.opt("characters")), run.optString("day", "")),
                positions(keys.optJSONArray("positions"))),
            badges);
    }

    // ---- 文案 ----

    /** 千分位：`12,846`。 */
    public static String grouped(long value) {
        return String.format(Locale.ROOT, "%,d", value);
    }

    /** 英雄卡下的周环比；上周没有记录时不写（没有可比的基数），返回 null。 */
    public static String weekDelta(long current, long previous) {
        if (previous <= 0) return null;
        long percent = Math.round((current - previous) * 100d / previous);
        if (percent == 0) return "和上周持平";
        return percent > 0 ? "比上周多 " + percent + "%" : "比上周少 " + -percent + "%";
    }

    /** 整数显示的速度；null 显示「—」。 */
    public static String whole(Double value) {
        return value == null ? "—" : String.valueOf(Math.round(value));
    }

    /** 0–1 的比例写成百分数的数字部分；null 显示「—」。 */
    public static String percent(Double rate) {
        return rate == null ? "—" : String.valueOf(Math.round(rate * 100));
    }

    /** 0–1 的比例写成一位小数的百分数数字部分（`7.4`）；null 显示「—」。 */
    public static String percentTenths(Double rate) {
        return rate == null ? "—" : tenths(rate * 100);
    }

    /** 一位小数（`2.3`），整数时不带 `.0`；null 显示「—」。 */
    public static String decimal(Double value) {
        return value == null ? "—" : tenths(value);
    }

    /** 平均速度的周对比；任一边没有数值时返回 null。 */
    public static String speedDelta(Double current, Double previous) {
        if (current == null || previous == null) return null;
        long difference = Math.round(current) - Math.round(previous);
        if (difference == 0) return "和上周一样快";
        return difference > 0 ? "比上周快 " + difference + " 字" : "比上周慢 " + -difference + " 字";
    }

    /** 每字按键的周对比；任一边没有数值时返回 null。 */
    public static String perKeyDelta(Double current, Double previous) {
        if (current == null || previous == null) return null;
        long tenths = Math.round(current * 10) - Math.round(previous * 10);
        if (tenths == 0) return "和上周持平";
        String amount = tenths(Math.abs(tenths) / 10d);
        return tenths < 0 ? "比上周少 " + amount + " 次" : "比上周多 " + amount + " 次";
    }

    /** 高峰时段：`晚上 9–11 点`；没有时返回 null。 */
    public static String peakLabel(PeakWindow window) {
        if (window == null) return null;
        int start = Math.floorMod(window.start(), 24);
        int end = Math.floorMod(window.end(), 24);
        return period(start) + " " + clock(start) + "–" + clock(end) + " 点";
    }

    /** `2026-09-28` 写成 `9 月 28 日`；解析不了时原样返回。 */
    public static String monthDay(String day) {
        if (day == null || day.length() != 10) return day == null ? "" : day;
        try {
            int month = Integer.parseInt(day.substring(5, 7));
            int date = Integer.parseInt(day.substring(8, 10));
            return month + " 月 " + date + " 日";
        } catch (NumberFormatException error) {
            return day;
        }
    }

    /** 徽章下的一行：已解锁写说明，未解锁时能算出差多少就写「还差 …」，否则仍写说明。 */
    public static String caption(Achievement badge) {
        if (badge.unlocked()) return badge.description();
        long missing = BoundsPolicy.nonNegative(badge.target() - badge.current());
        String unit = unit(badge.id());
        if (unit == null || missing == 0) return badge.description();
        if ("字".equals(unit)) return "还差 " + characters(missing);
        return "还差 " + missing + " " + unit;
    }

    /** 点按徽章后的提示：已解锁「名」· 说明，未解锁「名」· 要求。 */
    public static String toast(Achievement badge) {
        return badge.unlocked()
            ? "已解锁「" + badge.title() + "」· " + badge.description()
            : "「" + badge.title() + "」· " + caption(badge);
    }

    /** 环形进度中间的百分数，向下取整，免得差一点也显示成 100%。 */
    public static String progressLabel(Achievement badge) {
        return (int) Math.floor(badge.progress() * 100) + "%";
    }

    /** 输入构成：汉字、英文、符号、表情，其余文字和历史未分类合成「其他」；只列有字的段。 */
    public static List<Share> composition(Map<String, Long> characters) {
        Map<String, Long> groups = new LinkedHashMap<>(5);
        groups.put("汉字", value(characters, "han"));
        groups.put("英文", value(characters, "latin"));
        groups.put("符号", value(characters, "number") + value(characters, "punctuation")
            + value(characters, "symbol"));
        groups.put("表情", value(characters, "emoji"));
        groups.put("其他", value(characters, "otherLetter") + value(characters, "unknown"));
        return shares(groups);
    }

    /** 输入方式：26 键、9 键、语音、手写；AI 润色、回复和未分类不算输入方式，不计入。 */
    public static List<Share> methods(Map<String, Long> sources) {
        long nine = value(sources, "nineKey");
        long voice = value(sources, "voice");
        long handwriting = value(sources, "handwriting");
        long full = 0;
        for (Map.Entry<String, Long> entry : sources.entrySet()) {
            switch (entry.getKey()) {
                case "nineKey", "voice", "handwriting", "unknown", "ai", "reply" -> { }
                default -> full += BoundsPolicy.nonNegative(entry.getValue());
            }
        }
        Map<String, Long> groups = new LinkedHashMap<>(4);
        groups.put("26 键", full);
        groups.put("9 键", nine);
        groups.put("语音", voice);
        groups.put("手写", handwriting);
        return shares(groups);
    }

    /** 一组占比的总数。 */
    public static long total(List<Share> shares) {
        long total = 0;
        for (Share share : shares) total += share.count();
        return total;
    }

    /** `count / total` 写成整数百分数；总数为 0 时为 `0`。 */
    public static String share(long count, long total) {
        return total <= 0 ? "0" : String.valueOf(Math.round(count * 100d / total));
    }

    // ---- 内部 ----

    private static List<Share> shares(Map<String, Long> groups) {
        List<Share> result = new ArrayList<>(groups.size());
        for (Map.Entry<String, Long> entry : groups.entrySet()) {
            if (entry.getValue() > 0) result.add(new Share(entry.getKey(), entry.getValue()));
        }
        return List.copyOf(result);
    }

    private static long value(Map<String, Long> values, String key) {
        Long value = values.get(key);
        return value == null ? 0 : BoundsPolicy.nonNegative(value);
    }

    /** 各徽章进度的单位；速度、命中率、早起鸟这类阈值不是「差几个」能说清的，返回 null。 */
    private static String unit(String id) {
        return switch (id) {
            case "chars_10k", "chars_100k", "chars_1m", "night_owl", "shuangpin_10k",
                 "handwriting_500" -> "字";
            case "streak_7", "streak_30", "streak_100" -> "天";
            case "sentence_1000" -> "次";
            case "voice_1h" -> "分钟";
            case "words_50" -> "个词";
            case "skins_5" -> "款皮肤";
            default -> null;
        };
    }

    /** 字数：一万以上写成 `51.7 万字`。 */
    private static String characters(long count) {
        if (count >= 10_000) return tenths(count / 10_000d) + " 万字";
        return count + " 字";
    }

    private static String tenths(double value) {
        long rounded = Math.round(value * 10);
        if (rounded % 10 == 0) return String.valueOf(rounded / 10);
        return String.format(Locale.ROOT, "%.1f", rounded / 10d);
    }

    private static String period(int hour) {
        if (hour < 5) return "凌晨";
        if (hour < 8) return "早上";
        if (hour < 11) return "上午";
        if (hour < 13) return "中午";
        if (hour < 18) return "下午";
        if (hour < 19) return "傍晚";
        return "晚上";
    }

    private static int clock(int hour) {
        if (hour == 0) return 12;
        return hour > 12 ? hour - 12 : hour;
    }

    private static String text(JSONObject object, String key) {
        if (object.isNull(key)) return null;
        String value = object.optString(key, "");
        return value.isEmpty() ? null : value;
    }

    private static Double number(JSONObject object, String key) {
        if (object.isNull(key)) return null;
        Object value = object.opt(key);
        if (!(value instanceof Number number)) return null;
        double result = number.doubleValue();
        return Double.isFinite(result) ? result : null;
    }

    private static long count(Object value) {
        if (!(value instanceof Number number)) return 0;
        return BoundsPolicy.nonNegative(number.longValue());
    }

    private static List<DayCount> days(JSONArray array) {
        if (array == null) return List.of();
        List<DayCount> result = new ArrayList<>(array.length());
        for (int index = 0; index < array.length(); index++) {
            JSONObject entry = array.optJSONObject(index);
            if (entry == null) continue;
            result.add(new DayCount(entry.optString("day", ""), count(entry.opt("count"))));
        }
        return List.copyOf(result);
    }

    private static List<Long> hours(JSONArray array) {
        List<Long> result = new ArrayList<>(24);
        for (int hour = 0; hour < 24; hour++) {
            result.add(array == null || hour >= array.length() ? 0L : count(array.opt(hour)));
        }
        return List.copyOf(result);
    }

    private static List<Double> positions(JSONArray array) {
        if (array == null || array.length() != 4) return null;
        List<Double> result = new ArrayList<>(4);
        for (int index = 0; index < 4; index++) {
            Object value = array.opt(index);
            result.add(value instanceof Number number ? number.doubleValue() : 0d);
        }
        return List.copyOf(result);
    }

    private static Map<String, Long> counts(JSONObject object) {
        if (object == null) return Map.of();
        Map<String, Long> result = new LinkedHashMap<>(object.length());
        Iterator<String> keys = object.keys();
        while (keys.hasNext()) {
            String key = keys.next();
            result.put(key, count(object.opt(key)));
        }
        return Collections.unmodifiableMap(result);
    }
}
