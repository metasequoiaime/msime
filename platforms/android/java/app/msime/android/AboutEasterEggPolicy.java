package app.msime.android;

import java.time.LocalDate;
import java.time.temporal.ChronoUnit;

/** 关于页隐藏彩蛋的日期和气泡文案。 */
public final class AboutEasterEggPolicy {
    private static final LocalDate MOVED_IN = LocalDate.of(2026, 6, 6);

    private AboutEasterEggPolicy() {}

    /** 返回从入住日开始经过的自然日数，设备时钟回拨时保持为零。 */
    public static long daysTogether(LocalDate today) {
        return daysTogether(MOVED_IN, today);
    }

    public static long daysTogether(LocalDate movedIn, LocalDate today) {
        return Math.max(0, ChronoUnit.DAYS.between(movedIn, today));
    }

    /** 生成气泡文案；日期由调用方传入，便于测试和避免跨午夜前后不一致。 */
    public static String message(LocalDate today) {
        return "2026年6月6日住进你的手机中，截止到此时此刻你我已经相处 "
            + daysTogether(today) + " 天了。";
    }
}
