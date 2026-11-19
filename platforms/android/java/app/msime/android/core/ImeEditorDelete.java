package app.msime.android;

import android.os.SystemClock;
import android.view.KeyEvent;

/**
 * 交给编辑器的退格（`KEYCODE_DEL`）从发出到编辑器确认这一段：按键事件和 `InputConnection` 的写入在应用侧不保证先后（见 {@link BackspaceSettle}），所以输入法自己的写入回声没到时退格先等；发出后到编辑器回报选区（或 100 ms 超时）之前，根容器（{@link TouchHoldLayout}）按住新开始的触摸，连删的下一次也排队。
 *
 * <p>输入法服务只在删除入口、选区回报和换输入框时调用这里，状态都放在这个类里。
 */
final class ImeEditorDelete {
    private final MSIMEInputService s;
    private final BackspaceSettle settle = new BackspaceSettle();
    private final Runnable check = this::check;
    /** 有退格在路上时又要删一次（连删到点，或在等先前写入的回声）：确认后补这一次。 */
    private boolean queued;

    ImeEditorDelete(MSIMEInputService s) {
        this.s = s;
    }

    /** 有退格在路上（在等或发出未确认）：根容器按住新开始的触摸。 */
    boolean holding() {
        return settle.holding(SystemClock.uptimeMillis());
    }

    /** 上一次交给编辑器的退格还没确认：这一次排在它后面，确认后再删（连删的下一次就是这样），返回 true；否则返回 false，调用方照常删除。 */
    boolean queueBehindPending() {
        if (!holding()) return false;
        queued = true;
        schedule();
        return true;
    }

    /** 发 `KEYCODE_DEL` 让编辑器删光标前的内容；`before` 是刚读到的光标前文字（读不到时为 null）。 */
    void send(CharSequence before) {
        long now = SystemClock.uptimeMillis();
        s.imeLetterRows.invalidateBackspaceScrub();
        if (!settle.request(now, s.selectionEcho.hasPending(), s.selectionEcho.expectations())) {
            queued = true;
            schedule();
            return;
        }
        // 光标前没有内容时编辑器不回报（空框里删的是应用的引用块等），不记预期；读不到光标前的文字时算不出范围，和以前一样作废；
        // 否则记一条「左移 1 到读到的长度个单元、没有组字区」的范围预期。
        if (before == null) {
            s.selectionEcho.invalidate();
        } else if (before.length() > 0) {
            s.selectionEcho.deleteBeforeOpaque(before.length());
            s.selectionEcho.expect();
        }
        s.sendEditorKeyEvents(KeyEvent.KEYCODE_DEL);
        settle.sent(now);
        schedule();
    }

    /** 编辑器回报了选区：发出的退格就此确认，在等先前写入回声的退格也可能可以发了。 */
    void selectionReported() {
        if (settle.echo() || settle.ready(SystemClock.uptimeMillis(), s.selectionEcho.hasPending())) settled();
    }

    /** 换了输入框或连接：放弃在路上的退格，按住的触摸照常放出。 */
    void abandon() {
        s.main.removeCallbacks(check);
        settle.reset();
        queued = false;
        release();
    }

    private void schedule() {
        s.main.removeCallbacks(check);
        long deadline = settle.deadline();
        if (deadline >= 0) s.main.postAtTime(check, deadline);
    }

    /** 等待或确认的超时点到了。 */
    private void check() {
        long now = SystemClock.uptimeMillis();
        if (settle.expire(now)) {
            // 编辑器没回报（空框，或应用不回报）：这条范围预期不再可信，以后按外部回报重新定基，和以前发按键后一样。
            s.selectionEcho.invalidate();
            settled();
        } else if (settle.ready(now, s.selectionEcho.hasPending())) {
            settled();
        } else {
            schedule();
        }
    }

    /** 退格确认（或可以发了）：先补排着的那次删除，再按原顺序放出按住的触摸。 */
    private void settled() {
        s.main.removeCallbacks(check);
        if (queued) {
            queued = false;
            if (s.connection != null) s.deleteCodePointBeforeCursor();
        }
        release();
    }

    private void release() {
        if (s.touchHold != null) s.touchHold.release();
    }
}
