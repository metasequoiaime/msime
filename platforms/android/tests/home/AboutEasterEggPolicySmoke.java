import app.msime.android.AboutEasterEggPolicy;
import java.time.LocalDate;

/** 彩蛋日期文案只按本地日期计算，不因时区时间而多算或少算一天。 */
public final class AboutEasterEggPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        LocalDate movedIn = LocalDate.of(2026, 6, 6);
        check(AboutEasterEggPolicy.daysTogether(movedIn, movedIn) == 0,
            "入住当天是第 0 天");
        check(AboutEasterEggPolicy.daysTogether(movedIn, LocalDate.of(2026, 6, 7)) == 1,
            "第二天只增加一天");
        check(AboutEasterEggPolicy.daysTogether(movedIn, LocalDate.of(2026, 10, 27)) == 143,
            "跨月日期按自然日计算");
        check(AboutEasterEggPolicy.daysTogether(movedIn, LocalDate.of(2026, 6, 5)) == 0,
            "时钟回拨时不显示负天数");
        check(AboutEasterEggPolicy.message(LocalDate.of(2026, 10, 27))
                .contains("已经相处 143 天"),
            "气泡包含经过天数");
    }
}
