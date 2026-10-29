import app.msime.android.AndroidLocalSettings;
import app.msime.android.ClipboardUndoPolicy;

/** #10073：剪贴板多选删除后的撤销时长设置（5 到 9 秒，默认 5 秒）与浮动「撤销」的倒数。 */
public final class ClipboardUndoPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        AndroidLocalSettings.Snapshot defaults = AndroidLocalSettings.defaults();
        check(defaults.integer(AndroidLocalSettings.CLIPBOARD_UNDO_SECONDS) == 5,
            "undo stays 5 seconds until the user changes it");
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.CLIPBOARD_UNDO_SECONDS);
        check(!spec.synced, "the undo duration is local only: the sync field table does not know it");
        for (int seconds = 5; seconds <= 9; seconds++)
            check(Integer.valueOf(seconds).equals(spec.accept(seconds)), seconds + " seconds is accepted");
        check(spec.accept(4) == null && spec.accept(10) == null && spec.accept(0) == null,
            "durations outside 5..9 are refused");
        check(spec.accept(5.5) == null && spec.accept("7") == null && spec.accept(Boolean.TRUE) == null,
            "non-integer values are refused");

        check(ClipboardUndoPolicy.seconds(7) == 7 && ClipboardUndoPolicy.seconds(9) == 9, "valid durations pass");
        check(ClipboardUndoPolicy.seconds(4) == 5 && ClipboardUndoPolicy.seconds(10) == 5
                && ClipboardUndoPolicy.seconds(0) == 5 && ClipboardUndoPolicy.seconds(-1) == 5,
            "an out-of-range duration falls back to 5 seconds");

        // 7 秒期限：刚删除时显示 7，过了 0.1 秒仍是 7，过了 1 秒整变 6，最后一刻是 1，到截止归零。
        long now = 1_000_000L;
        long deadline = ClipboardUndoPolicy.deadline(now, 7);
        check(deadline == now + 7000L, "the deadline is the configured seconds after the deletion");
        check(ClipboardUndoPolicy.deadline(now, 42) == now + 5000L, "a bad duration still expires after 5 seconds");
        check(ClipboardUndoPolicy.remainingSeconds(deadline, now) == 7, "the countdown starts at the full duration");
        check(ClipboardUndoPolicy.remainingSeconds(deadline, now + 100) == 7, "partial seconds round up");
        check(ClipboardUndoPolicy.remainingSeconds(deadline, now + 1000) == 6, "one second later it shows 6");
        check(ClipboardUndoPolicy.remainingSeconds(deadline, deadline - 1) == 1, "the last moment still shows 1");
        check(ClipboardUndoPolicy.remainingSeconds(deadline, deadline) == 0
                && ClipboardUndoPolicy.remainingSeconds(deadline, deadline + 500) == 0,
            "the countdown is zero at and after the deadline");

        // 下一次刷新正好落在显示的数字变化的那一刻，倒数不会跳过一秒或在同一个数字上停两次。
        check(ClipboardUndoPolicy.millisUntilNextTick(deadline, now) == 1000, "a whole second until the first tick");
        check(ClipboardUndoPolicy.millisUntilNextTick(deadline, now + 300) == 700,
            "a late start ticks when the shown number changes");
        check(ClipboardUndoPolicy.millisUntilNextTick(deadline, deadline - 200) == 200,
            "the last tick lands on the deadline");
        check(ClipboardUndoPolicy.millisUntilNextTick(deadline, deadline) == 0
                && ClipboardUndoPolicy.millisUntilNextTick(deadline, deadline + 10) == 0,
            "no further tick after the deadline");
        long at = now + 300;
        int shown = ClipboardUndoPolicy.remainingSeconds(deadline, at);
        int ticks = 0;
        while (shown > 0) {
            at += ClipboardUndoPolicy.millisUntilNextTick(deadline, at);
            int next = ClipboardUndoPolicy.remainingSeconds(deadline, at);
            check(next == shown - 1, "each tick lowers the countdown by exactly one: " + shown + " -> " + next);
            shown = next;
            ticks++;
        }
        check(ticks == 7 && at == deadline, "seven ticks bring the countdown to zero exactly at the deadline");

        check("撤销（5 秒）".equals(ClipboardUndoPolicy.buttonLabel(5)), "button label shows the remaining seconds");
        check(ClipboardUndoPolicy.buttonDescription(3).startsWith("撤销刚才删除的剪贴板记录")
                && ClipboardUndoPolicy.buttonDescription(3).contains("3 秒"),
            "the accessibility description names the action and the remaining time");
        check("已删除 2 项，可在 9 秒内撤销".equals(ClipboardUndoPolicy.deletedNotice(2, 9)),
            "the deletion notice uses the configured duration");
        check("8 秒".equals(ClipboardUndoPolicy.settingLabel(8)), "setting label");
        System.out.println("ClipboardUndoPolicySmoke ok");
    }
}
