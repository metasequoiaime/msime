package app.msime.android;

/**
 * 剪贴板多选删除后的撤销期限（#10073）：本地设置 `platform.android.clipboard_undo_seconds`，5 到 9 秒，默认 5 秒。
 *
 * <p>删除后剪贴板面板右下角浮着一个大号「撤销」按钮，按钮上倒数剩下的秒数，归零时连同可撤销的记录一起消失。这里只放和视图无关的计时与文案，键盘里的接线在 `MSIMEInputService.removeClipboardItems` 和 `ImePanels.updateClipboardUndo`。
 */
public final class ClipboardUndoPolicy {
    public static final int MIN_SECONDS = 5;
    public static final int MAX_SECONDS = 9;
    public static final int DEFAULT_SECONDS = 5;

    private ClipboardUndoPolicy() {}

    /** 实际使用的期限：不在 5..9 内的值按默认 5 秒。设置文件读出的值已经由 `AndroidLocalSettings` 校验过，这里再兜一次，期限不会因为调用方传错而变成 0 或很长。 */
    public static int seconds(int configured) {
        return configured < MIN_SECONDS || configured > MAX_SECONDS ? DEFAULT_SECONDS : configured;
    }

    /** 删除那一刻起算的截止时刻，时钟与 `now` 同源（`SystemClock.uptimeMillis`）。 */
    public static long deadline(long now, int seconds) {
        return now + seconds(seconds) * 1000L;
    }

    /** 还剩几秒，向上取整：截止前 4.2 秒显示 5，刚过截止为 0，0 表示撤销已经过期。 */
    public static int remainingSeconds(long deadline, long now) {
        long left = deadline - now;
        return left <= 0 ? 0 : (int) ((left + 999) / 1000);
    }

    /** 下一次刷新倒数的延迟：正好落在显示的秒数减一的那一刻，最后一次落在截止时刻；已过期时为 0。 */
    public static long millisUntilNextTick(long deadline, long now) {
        long left = deadline - now;
        if (left <= 0) return 0;
        long partial = left % 1000;
        return partial == 0 ? 1000 : partial;
    }

    /** 浮动按钮上的字，例如「撤销（5 秒）」。 */
    public static String buttonLabel(int remaining) {
        return "撤销（" + remaining + " 秒）";
    }

    /** 浮动按钮的读屏说明：按钮上的字随倒数变化，说明里同样带上剩余秒数，读屏用户也知道还来不来得及。 */
    public static String buttonDescription(int remaining) {
        return "撤销刚才删除的剪贴板记录，还剩 " + remaining + " 秒";
    }

    /** 删除后的提示。 */
    public static String deletedNotice(int count, int seconds) {
        return "已删除 " + count + " 项，可在 " + seconds + " 秒内撤销";
    }

    /** 设置页上的取值，例如「5 秒」。 */
    public static String settingLabel(int seconds) {
        return seconds + " 秒";
    }
}
