package app.msime.android;

/**
 * 离开输入框后欠着的那一轮词库快照处理（排着的快照激活、待合并的输入记录）从什么时候开始欠的。
 *
 * <p>失焦后的处理延后 500 ms，下一个要建会话的输入框会取消它，好让新会话不排在整份用户词库的扫描后面。一直在同一个应用的几个输入框之间切换、从不闲下来时，每一次都会被取消；欠满 {@link #MAX_DEFER_MILLIS} 之后，下一次失焦就不再延后，像原来一样立即排进工作线程、换输入框也不取消，于是这样切换时最多每分钟让一个新会话等一次扫描。服务销毁时还欠着的那一轮在关掉工作线程前补上。
 *
 * <p>只记一个时刻，不碰文件。不是线程安全的：输入法服务只在主线程上用它。
 */
public final class DictionarySnapshotBacklog {
    /** 欠到这么久就不再延后。 */
    public static final long MAX_DEFER_MILLIS = 60_000;

    private long owedSince = -1;

    /**
     * 又一次失焦欠下处理，{@code now} 是单调时钟的毫秒数。
     *
     * @return 这一轮还能延后、可被下一个输入框取消时为真；欠满 {@link #MAX_DEFER_MILLIS} 时为假，这一轮要立即排队、不可取消。
     */
    public boolean owe(long now) {
        if (owedSince < 0) owedSince = now;
        return now - owedSince < MAX_DEFER_MILLIS;
    }

    /** 还欠着一轮处理。 */
    public boolean owed() {
        return owedSince >= 0;
    }

    /** 一轮处理已经做完，或已经不可取消地排进了工作线程；之后再失焦从头计时。 */
    public void settled() {
        owedSince = -1;
    }
}
