import app.msime.android.NumberPolicy;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.TypingStatisticsSummary.Achievement;
import app.msime.android.TypingStatisticsSummary.PeakWindow;
import app.msime.android.TypingStatisticsSummary.Share;
import java.util.List;
import java.util.Map;

/**
 * 统计页把 `summary` 的数字写成文案的规则。
 *
 * <p>解析本身用 org.json，这里的 android.jar 只有抛 `Stub!` 的桩，跑不了；文案规则和记录类型是纯 Java，在这里锁住。
 */
public final class TypingStatisticsSummarySmoke {
    public static void main(String[] args) {
        check("12,846".equals(NumberPolicy.grouped(12_846)), "thousands separator");
        check("比上周多 18%".equals(TypingStatisticsSummary.weekDelta(12_846, 10_886)), "week up");
        check("比上周少 50%".equals(TypingStatisticsSummary.weekDelta(50, 100)), "week down");
        check("和上周持平".equals(TypingStatisticsSummary.weekDelta(100, 100)), "week flat");
        check(TypingStatisticsSummary.weekDelta(100, 0) == null, "no previous week, no delta");

        check(TypingStatisticsSummary.strictCount(42L) == 42L,
            "statistics counts accept JSON integers");
        check(TypingStatisticsSummary.strictCount(1.5d) == 0L,
            "statistics counts reject fractional JSON numbers");
        check(TypingStatisticsSummary.strictCount(-1L) == 0L,
            "statistics counts reject negative JSON integers");
        check(TypingStatisticsSummary.strictPositionRate(0.25d) == 0.25d,
            "statistics position rates accept values in range");
        check(TypingStatisticsSummary.strictPositionRate(1.5d) == 0d,
            "statistics position rates reject values above one");
        check(TypingStatisticsSummary.strictPositionRate(-0.1d) == 0d,
            "statistics position rates reject negative values");
        check(TypingStatisticsSummary.strictRate(0.25d) == 0.25d,
            "summary rates accept values in range");
        check(TypingStatisticsSummary.strictRate(1.5d) == null,
            "summary rates reject values above one");
        check(TypingStatisticsSummary.strictRate(-0.1d) == null,
            "summary rates reject negative values");
        check("—".equals(TypingStatisticsSummary.whole(null)), "null speed is a dash");
        check("52".equals(TypingStatisticsSummary.whole(51.6)), "speed rounds");
        check("91".equals(TypingStatisticsSummary.percent(0.912)), "percent rounds");
        check("—".equals(TypingStatisticsSummary.percent(null)), "null rate is a dash");
        check("7.4".equals(TypingStatisticsSummary.percentTenths(0.0741)), "tenths percent");
        check("2".equals(TypingStatisticsSummary.decimal(2.04)), "whole decimal drops .0");
        check("比上周快 4 字".equals(TypingStatisticsSummary.speedDelta(52d, 48d)), "speed up");
        check("比上周慢 3 字".equals(TypingStatisticsSummary.speedDelta(45d, 48d)), "speed down");
        check(TypingStatisticsSummary.speedDelta(52d, null) == null, "speed without baseline");
        check("比上周少 0.2 次".equals(TypingStatisticsSummary.perKeyDelta(2.3, 2.5)), "keys down");
        check("比上周多 1 次".equals(TypingStatisticsSummary.perKeyDelta(3.5, 2.5)), "keys up");

        check("晚上 9–11 点".equals(TypingStatisticsSummary.peakLabel(new PeakWindow(21, 23))),
            "evening window");
        check("晚上 11–1 点".equals(TypingStatisticsSummary.peakLabel(new PeakWindow(23, 1))),
            "a window across midnight");
        check("下午 2–4 点".equals(TypingStatisticsSummary.peakLabel(new PeakWindow(14, 16))),
            "afternoon window");
        check(TypingStatisticsSummary.peakLabel(null) == null, "no window");
        check("9 月 28 日".equals(TypingStatisticsSummary.monthDay("2026-09-28")), "month day");

        Achievement million = new Achievement("chars_1m", "百万", "著作等身", "累计输入 100 万字",
            "volume", null, 483_000, 1_000_000);
        check("还差 51.7 万字".equals(TypingStatisticsSummary.caption(million)), "million caption");
        check("48%".equals(TypingStatisticsSummary.progressLabel(million)), "ring floors");
        check("「著作等身」· 还差 51.7 万字".equals(TypingStatisticsSummary.toast(million)),
            "locked toast names the requirement");
        Achievement streak = new Achievement("streak_30", "30", "连续 30 天", "连续使用 30 天",
            "streak", null, 23, 30);
        check("还差 7 天".equals(TypingStatisticsSummary.caption(streak)), "streak caption");
        Achievement first = new Achievement("chars_10k", "1万", "初出茅庐", "累计输入 1 万字",
            "volume", "2026-09-01", 12_000, 10_000);
        check(first.unlocked() && first.progress() == 1d, "unlocked is full");
        check("已解锁「初出茅庐」· 累计输入 1 万字".equals(TypingStatisticsSummary.toast(first)),
            "unlocked toast");
        Achievement speed = new Achievement("speed_60", "60", "快手", "平均每分钟 60 字", "skill",
            null, 52, 60);
        check("平均每分钟 60 字".equals(TypingStatisticsSummary.caption(speed)),
            "a threshold badge keeps its description");

        List<Share> mix = TypingStatisticsSummary.composition(Map.of("han", 82L, "latin", 11L,
            "punctuation", 3L, "number", 2L, "emoji", 2L));
        check(mix.size() == 4 && "汉字".equals(mix.get(0).title()) && mix.get(2).count() == 5,
            "composition groups symbols");
        check(TypingStatisticsSummary.total(mix) == 100, "composition total");
        check("82".equals(TypingStatisticsSummary.share(82, 100)), "share");
        check("0".equals(TypingStatisticsSummary.share(1, 0)), "share of nothing");

        List<Share> methods = TypingStatisticsSummary.methods(Map.of("quanpin", 60L, "english", 8L,
            "nineKey", 19L, "voice", 9L, "handwriting", 4L, "ai", 30L, "unknown", 5L));
        check(methods.size() == 4 && methods.get(0).count() == 68 && methods.get(1).count() == 19,
            "methods leave out AI and unknown");
        // 14 键单独一档，排在 26 键和 9 键之间，不再并进 26 键（与共享 UI 的 summaryMethods 一致）。
        List<Share> withFourteen = TypingStatisticsSummary.methods(Map.of("quanpin", 60L, "fourteenKey", 12L,
            "nineKey", 19L));
        check(withFourteen.size() == 3 && withFourteen.get(0).count() == 60
            && "14 键".equals(withFourteen.get(1).title()) && withFourteen.get(1).count() == 12
            && "9 键".equals(withFourteen.get(2).title()), "the fourteen-key keyboard is its own method");

        TypingStatisticsSummary summary = new TypingStatisticsSummary(null, null, null,
            List.of(first, million, streak));
        check(summary.unlockedCount() == 1, "unlocked count");
        System.out.println("TypingStatisticsSummarySmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
