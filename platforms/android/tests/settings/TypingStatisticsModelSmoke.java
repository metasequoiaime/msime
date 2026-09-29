import app.msime.android.TypingStatisticsModel;
import app.msime.android.TypingStatisticsModel.Section;
import app.msime.android.TypingStatisticsModel.Slice;
import java.util.List;
import java.util.Map;

/**
 * The model's arithmetic, on the shape the shared store serialises.
 *
 * <p>The document itself is decoded by TypingStatisticsDocument, which cannot run here: `org.json`
 * is a stub in the SDK jar. The maps below are what that decoder produces.
 */
public final class TypingStatisticsModelSmoke {
    private static TypingStatisticsModel model() {
        return new TypingStatisticsModel(true, 120, "90d",
            Map.of("2026-09-18", 20L, "2026-09-20", 100L),
            Map.of("han", 70L, "latin", 20L, "punctuation", 10L),
            Map.of("nineKey", 60L, "english", 20L, "voice", 10L, "handwriting", 10L),
            Map.of("2026-09-20", Map.of("han", 70L)),
            Map.of("2026-09-20", Map.of("nineKey", 60L)));
    }

    public static void main(String[] args) {
        TypingStatisticsModel model = model();
        check(model.enabled() && model.total() == 120 && "90d".equals(model.retention()),
            "headline fields");
        check(model.count("2026-09-20") == 100 && model.count("2026-09-19") == 0,
            "a day without a record counts zero");
        check("2026-09-20".equals(model.latestDay()), "latest recorded day");

        // The series is dense: the silent day between the two records has to be a point on the
        // chart, not a gap the line draws straight through.
        int[] trend = model.trend("2026-09-20", 4);
        check(trend.length == 4, "the series is as long as asked");
        check(trend[0] == 0 && trend[1] == 20 && trend[2] == 0 && trend[3] == 100,
            "the series is dense and ends today");
        check(model.trendDays("2026-09-20", 2).equals(List.of("2026-09-19", "2026-09-20")),
            "the day keys match the series");
        check(model.trend("2026-09-20", 0).length == 0, "an empty window has no points");
        check(model.trend("not a day", 3).length == 3, "an unreadable day still yields a window");
        check(model.trend("2026-09-20", 10_000).length == TypingStatisticsModel.MAX_TREND_DAYS,
            "the window is capped at the retained span");
        check(model.recordedSpan("2026-09-20") == 3, "the span reaches the earliest record");
        check(model.recordedSpan("2026-09-17") == 0, "a record in the future spans nothing");

        List<Slice> kinds = model.slices(Section.KIND, null);
        check(kinds.size() == 8 && "han".equals(kinds.get(0).id()), "character kinds are ordered");
        check(kinds.get(0).count() == 70, "character counts are read");
        // 100 classified of 120 recorded: the other 20 predate the breakdown and are not dropped.
        check(last(kinds).count() == 20 && "unknown".equals(last(kinds).id()),
            "the unclassified remainder is filed rather than lost");
        check(TypingStatisticsModel.sum(kinds) == model.total(), "kinds sum to the total");

        List<Slice> schemes = model.slices(Section.SCHEME, null);
        check(schemes.size() == 15 && "quanpin".equals(schemes.get(0).id()), "schemes are ordered");
        check(TypingStatisticsModel.sum(schemes) == model.total(), "schemes sum to the total");

        List<Slice> modes = model.slices(Section.MODE, null);
        check(count(modes, "chinese") == 60, "the pinyin schemes fold into one Chinese mode");
        check(count(modes, "english") == 20 && count(modes, "voice") == 10, "modes read their own");
        check(count(modes, "handwriting") == 10,
            "handwriting is its own mode rather than being dropped");
        check(TypingStatisticsModel.sum(modes) == model.total(), "modes sum to the total");

        List<Slice> day = model.slices(Section.KIND, "2026-09-20");
        check(count(day, "han") == 70 && count(day, "unknown") == 30,
            "a day scopes to its own detail and its own remainder");
        check(TypingStatisticsModel.sum(day) == model.count("2026-09-20"),
            "a day's slices sum to that day");
        check(TypingStatisticsModel.sum(model.slices(Section.KIND, "2026-09-18")) == 20,
            "a day with no detail is entirely unclassified");
        check(model.slices(Section.TREND, null).isEmpty(), "the trend is a series, not a pie");

        check("趋势".equals(Section.TREND.tab()) && "每日趋势".equals(Section.TREND.heading()),
            "the tab label is short and the heading is not");
        System.out.println("Android typing statistics model: series, scoping and distributions passed");
    }

    private static Slice last(List<Slice> slices) { return slices.get(slices.size() - 1); }

    private static long count(List<Slice> slices, String id) {
        for (Slice slice : slices) if (slice.id().equals(id)) return slice.count();
        throw new AssertionError("Missing slice: " + id);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
