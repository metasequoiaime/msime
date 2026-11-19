package app.msime.android;

import android.os.SystemClock;
import android.view.KeyEvent;

/**
 * 用户的退格交给编辑器（`KEYCODE_DEL`）这一路：何时改为直接删除，以及按键从发出到编辑器确认这一段。
 *
 * <p>按键事件和 `InputConnection` 的写入在应用侧不保证先后（见 {@link BackspaceSettle}），所以输入法自己的写入回声没到时退格先等；发出后到编辑器回报选区（或 100 ms 超时）之前，根容器（{@link TouchHoldLayout}）按住新开始的触摸，连删的下一次、横滑和松手的快速删除也等确认。确认后读回核对删掉了什么，决定能否并进横滑恢复（{@link EditorDeleteCheck}）。
 *
 * <p>输入法服务只在删除入口、选区回报、横滑与快速删除入口和换输入框时调用这里，状态都放在这个类里；直接按字素删除的实现留在服务里（{@link MSIMEInputService#deleteGraphemeBefore}）。
 */
final class ImeEditorDelete {
    /** 超时之后再等这么久读一次，文字仍原样不动才算编辑器不处理按键：慢的应用只是晚到，不能当成不处理而再删一次。 */
    private static final long IGNORED_GRACE_MS = 400;

    private final MSIMEInputService s;
    private final BackspaceSettle settle = new BackspaceSettle();
    private final Runnable check = this::check;
    private final Runnable ignoredCheck = this::checkIgnored;
    /** 有退格在路上时又要删一次（连删到点，或在等先前写入的回声）：确认后补这一次。 */
    private boolean queued;
    /** 在路上的那次退格发出前读到的光标前文字：确认后核对删掉了什么，超时后判断编辑器是不是不处理按键。 */
    private CharSequence before;
    /** 当前输入框不处理退格按键（发出后等满确认时间、再等一会儿，文字原样不动）：这次输入会话里改为直接按字素删除。换输入框时复位。 */
    private boolean ignoresKeys;
    /** 退格未确认时到来的横滑恢复：只记最后一个目标，确认后再移动（横滑的删除和恢复都是读回核实的直接写入）。 */
    private BackspaceScrub pendingScrub;
    private int pendingScrubDeleted;
    /** 退格未确认时松手要做的快速删除或撤回，确认后再做。 */
    private Runnable pendingAfter;

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

    /**
     * 这次用户退格改为直接按字素删除：编辑器不处理按键，或者光标前以拉丁类组合附加符号结尾（唯一的例外，系统 `EditText` 的按键退格只删附加符号，见 {@link EditorDeleteCheck#endsWithLatinCombiningMark}）。
     */
    boolean deletesDirectly(CharSequence before) {
        return ignoresKeys || EditorDeleteCheck.endsWithLatinCombiningMark(before);
    }

    /** 发 `KEYCODE_DEL` 让编辑器删光标前的内容；`before` 是刚读到的光标前文字（读不到时为 null）。 */
    void send(CharSequence before) {
        long now = SystemClock.uptimeMillis();
        if (!settle.request(now, s.selectionEcho.hasPending(), s.selectionEcho.expectations())) {
            queued = true;
            schedule();
            return;
        }
        s.main.removeCallbacks(ignoredCheck);
        this.before = before == null ? null : before.toString();
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
        if (settle.echo()) {
            record();
            settled();
        } else if (settle.ready(SystemClock.uptimeMillis(), s.selectionEcho.hasPending())) {
            settled();
        }
    }

    /** 退格未确认时横滑到了新位置：记下最后的目标，确认后再移动，返回 true；横滑接管后连删的下一次不再补。 */
    boolean deferScrub(BackspaceScrub scrub, int deleted) {
        if (!holding()) return false;
        pendingScrub = scrub;
        pendingScrubDeleted = deleted;
        queued = false;
        schedule();
        return true;
    }

    /** 有退格未确认时把 `action`（松手的快速删除、撤回）留到确认之后（只留最后一个），返回 true；否则返回 false，调用方照常执行。 */
    boolean defer(Runnable action) {
        if (!holding()) return false;
        pendingAfter = action;
        schedule();
        return true;
    }

    /** 换了输入框或连接：放弃在路上的退格和等着的横滑、快速删除，按住的触摸照常放出；新输入框重新判断是否处理按键。 */
    void abandon() {
        s.main.removeCallbacks(check);
        s.main.removeCallbacks(ignoredCheck);
        settle.reset();
        queued = false;
        before = null;
        ignoresKeys = false;
        pendingScrub = null;
        pendingAfter = null;
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
            // 删了什么不知道，横滑恢复作废；光标前原本有文字时，过一会儿再看编辑器是不是根本不处理按键。
            s.imeLetterRows.invalidateBackspaceScrub();
            if (before != null && before.length() > 0) s.main.postDelayed(ignoredCheck, IGNORED_GRACE_MS);
            else before = null;
            settled();
        } else if (settle.ready(now, s.selectionEcho.hasPending())) {
            settled();
        } else {
            schedule();
        }
    }

    /** 确认后核对删掉了什么：恰好一个普通码位就并进这次按压的横滑恢复（和直接删除时一样），否则作废恢复。 */
    private void record() {
        CharSequence previous = before;
        before = null;
        if (previous == null || !s.imeLetterRows.hasBackspaceScrub()) return;
        CharSequence after = EditorBackspaceBoundary.before(s.connection);
        if (EditorDeleteCheck.onePlainCodePointRemoved(previous, after, EditorBackspaceBoundary.CONTEXT_LENGTH))
            s.imeLetterRows.recordBackspaceScrubDelete(previous);
        else s.imeLetterRows.invalidateBackspaceScrub();
    }

    /** 超时之后又等了一会儿：光标前的文字和发出退格前完全一样，就是编辑器不处理按键，改为直接删除并补上这一次。 */
    private void checkIgnored() {
        CharSequence previous = before;
        before = null;
        if (previous == null || s.connection == null || holding()) return;
        CharSequence now = EditorBackspaceBoundary.before(s.connection);
        if (now == null || !previous.toString().equals(now.toString())) return;
        ignoresKeys = true;
        if (s.prepareDelete()) s.deleteGraphemeBefore(now);
    }

    /** 退格确认（或可以发了）：先补排着的那次删除；它又发出了按键就等下一次确认。然后移动等着的横滑、做松手留下的快速删除，再按原顺序放出按住的触摸。 */
    private void settled() {
        s.main.removeCallbacks(check);
        if (queued) {
            queued = false;
            if (s.connection != null) s.deleteCodePointBeforeCursor();
        }
        if (holding()) return;
        if (pendingScrub != null) {
            BackspaceScrub scrub = pendingScrub;
            pendingScrub = null;
            s.moveBackspaceScrub(scrub, pendingScrubDeleted);
        }
        if (pendingAfter != null) {
            Runnable action = pendingAfter;
            pendingAfter = null;
            action.run();
        }
        release();
    }

    private void release() {
        if (s.touchHold != null) s.touchHold.release();
    }
}
