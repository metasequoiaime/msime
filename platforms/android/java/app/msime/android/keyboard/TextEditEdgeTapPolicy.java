package app.msime.android;

/**
 * 文本编辑面板 ⇤ ⇥ 的单击和双击（#11111）：单击移到行首、行尾，立即执行，不等第二下；时间窗口内对同一个键再按一下，接着移到整篇的开头、结尾。超时的第二下、换了另一个键的一下都按单击算。做法同 {@link ShiftTapPolicy}。
 */
public final class TextEditEdgeTapPolicy {
    /** 双击的时间窗口，暂取 Shift 双击锁定的 350 ms，实测后再定。 */
    public static final long DOUBLE_TAP_MS = ShiftTapPolicy.DOUBLE_TAP_MS;

    private TextEditPanelModel.Action lastKey;
    private long lastTapAt = Long.MIN_VALUE;

    /**
     * 按一下 ⇤ 或 ⇥，返回这一下要执行的动作。
     *
     * @param key {@code DOCUMENT_START}（⇤）或 {@code DOCUMENT_END}（⇥）
     * @param nowMs 这一下的时间，和上一下同一个时钟
     * @return 单击给 {@code LINE_START} / {@code LINE_END}，双击的第二下给 {@code DOCUMENT_START} / {@code DOCUMENT_END}；不是这两个键时原样返回
     */
    public TextEditPanelModel.Action tap(TextEditPanelModel.Action key, long nowMs) {
        TextEditPanelModel.Action line = switch (key) {
            case DOCUMENT_START -> TextEditPanelModel.Action.LINE_START;
            case DOCUMENT_END -> TextEditPanelModel.Action.LINE_END;
            default -> null;
        };
        if (line == null) {
            reset();
            return key;
        }
        boolean second = key == lastKey && lastTapAt != Long.MIN_VALUE
            && nowMs >= lastTapAt && nowMs - lastTapAt <= DOUBLE_TAP_MS;
        if (second) {
            // 第二下用掉这次双击：紧接着的第三下重新从单击算。
            reset();
            return key;
        }
        lastKey = key;
        lastTapAt = nowMs;
        return line;
    }

    /** 按了别的键、换了输入框或面板关掉时调用，下一下 ⇤ ⇥ 重新从单击算。 */
    public void reset() {
        lastKey = null;
        lastTapAt = Long.MIN_VALUE;
    }
}
