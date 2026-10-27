package app.msime.android;

/**
 * 双击选词、三击选行回读的代次，只在主线程使用。新的触摸、换编辑器和编辑器报告的选区变化都让在途回读作废，但触控板自己刚设下的选区回报回来时不算外部变化。
 *
 * <p>快速三击时第二下的选词结果先写进编辑器，第三下按下、松手并发出选行回读；编辑器在答复这次回读之前已经发出选词的选区回报，主线程却可能在第三下松手之后才处理它。把这份回报当成外部改动，就会丢掉已经读到这段选区的选行结果，最后只选中一个词。
 */
public final class TextTrackpadSelectionVersion {
    private long version;
    private int appliedStart = -1;
    private int appliedEnd = -1;

    public long current() { return version; }

    /** 新的一下触摸开始：在途回读作废，尚未回报的自身选区仍然有效。 */
    public void invalidate() { version++; }

    /** 编辑器已接受触控板设下的选区；它的回报只认一次。 */
    public void applied(int start, int end) {
        appliedStart = start;
        appliedEnd = end;
    }

    /** 编辑器报告的选区：自身选区的回报不作废回读，其余一律视为外部变化。 */
    public void reported(int start, int end) {
        if (appliedStart >= 0 && start == appliedStart && end == appliedEnd) {
            appliedStart = appliedEnd = -1;
            return;
        }
        version++;
    }

    /** 换编辑器、关闭面板：在途回读和尚未回报的自身选区一起作废。 */
    public void reset() {
        version++;
        appliedStart = appliedEnd = -1;
    }
}
