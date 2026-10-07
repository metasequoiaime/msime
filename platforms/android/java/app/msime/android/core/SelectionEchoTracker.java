package app.msime.android;

/**
 * 区分编辑器迟到的选区回报和用户真正移动了光标，不依赖 Android，便于在 JVM 上测试。
 *
 * <p>`onUpdateSelection` 是编辑器所在进程异步发回来的。输入法刚上屏一段文字（空格、点候选、标点），编辑器稍后才报告「光标在 N、没有组字区」；如果这时用户已经按下一个键开始了新的组字，这条迟到的回报看上去就像光标离开了组字区，旧逻辑会把新组字整个取消——九键里刚按的数字直接消失，26 键里刚按的字母变成原文留在框里。九键从不在编辑器里写组字区，`composingEnd` 一直是 -1，所以任何迟到的回报都会触发。
 *
 * <p>做法和 LatinIME 的 expected selection 一样：输入法每次自己写编辑器，都按 `InputConnection` 的语义算出写完后的选区和组字区，记成一条待确认的预期。回报的选区和组字区都和某条预期相同，就是那次写入的回声，消掉它和更早的预期，不当作光标移动；对不上任何预期的才是外部变化（用户点了别处、应用自己改了文字或去掉了组字区），这时以回报为准重新定基，组字照旧取消。
 *
 * <p>按码位删除光标前一个字符时，删掉的是一个还是两个 UTF-16 单元在这里算不出来（要读编辑器里的文字，又是一次跨进程调用），所以预期带一个不确定量 `slack`：真实位置是记下的位置整体减去 0 到 `slack` 之间的同一个数。回声对上时就知道减的是几，后面的预期据此收窄。代价是外部移动恰好落进这个一两个单元的范围时会被当成回声；为此只在光标折叠、没有组字区、离文档开头足够远时预测删除，不确定量也有上限，超出就退回不预测。
 *
 * <p>除此之外只会少取消、不会多取消：预期算错（应用改写了上屏的文字等）只会对不上，退回原来的行为。写入结果无法预测时调 {@link #invalidate()}，在下一次外部回报重新定基之前不再记预期。
 */
public final class SelectionEchoTracker {
    private static final int MAX_PENDING = 16;
    /** 连续按码位删除而回声还没回来时，不确定量最多累计到这么多个单元，再多就不预测。 */
    private static final int MAX_SLACK = 4;

    private boolean known;
    private int selectionStart;
    private int selectionEnd;
    private int composingStart = -1;
    private int composingEnd = -1;
    private int slack;
    private boolean written;
    private final int[] pendingStart = new int[MAX_PENDING];
    private final int[] pendingEnd = new int[MAX_PENDING];
    private final int[] pendingComposingStart = new int[MAX_PENDING];
    private final int[] pendingComposingEnd = new int[MAX_PENDING];
    private final int[] pendingSlack = new int[MAX_PENDING];
    private int pendingCount;

    /** 新的编辑器：从 `EditorInfo.initialSelStart/End` 开始，任何一端未知（-1）就先不预测。 */
    public void reset(int start, int end) {
        pendingCount = 0;
        written = false;
        composingStart = -1;
        composingEnd = -1;
        slack = 0;
        known = start >= 0 && end >= 0;
        selectionStart = BoundsPolicy.atMost(start, end);
        selectionEnd = BoundsPolicy.atLeast(start, end);
    }

    /** 这次写入的结果算不出来（发按键事件、编辑器拒绝了写入、删除时有选区或组字区等）。 */
    public void invalidate() {
        pendingCount = 0;
        known = false;
        written = false;
        slack = 0;
    }

    /** `commitText(text, 1)` 成功：替换组字区，没有组字区时替换选区，光标停在新文字之后。`length` 是 UTF-16 长度，和选区下标同一单位。 */
    public void commit(int length) {
        if (!known) return;
        int start = replacedStart();
        selectionStart = start + length;
        selectionEnd = selectionStart;
        composingStart = -1;
        composingEnd = -1;
        written = true;
    }

    /** `setComposingText(text, 1)` 成功：同样替换组字区或选区，新文字成为组字区，光标停在它之后。 */
    public void compose(int length) {
        if (!known) return;
        int start = replacedStart();
        selectionStart = start + length;
        selectionEnd = selectionStart;
        composingStart = length == 0 ? -1 : start;
        composingEnd = length == 0 ? -1 : start + length;
        written = true;
    }

    /** `finishComposingText()` 成功：文字和光标不动，只去掉组字区。 */
    public void finish() {
        if (!known) return;
        composingStart = -1;
        composingEnd = -1;
        written = true;
    }

    /** `deleteSurroundingText(length, 0)` 成功：光标折叠且没有组字区时光标左移 `length` 个 UTF-16 单元，否则算不出来。 */
    public void deleteBefore(int length) {
        shiftLeft(length, length);
    }

    /** `deleteSurroundingTextInCodePoints(1, 0)` 成功：光标左移一个或两个 UTF-16 单元（代理对），哪一个由回声决定。 */
    public void deleteCodePointBefore() {
        shiftLeft(1, 2);
    }

    /** 一批写入结束（`endBatchEdit` 或一次单独的写入之后）：把写完后的选区和组字区记成一条待确认的预期。 */
    public void expect() {
        if (!known || !written) return;
        written = false;
        if (pendingCount == MAX_PENDING) {
            drop(1);
        }
        pendingStart[pendingCount] = selectionStart;
        pendingEnd[pendingCount] = selectionEnd;
        pendingComposingStart[pendingCount] = composingStart;
        pendingComposingEnd[pendingCount] = composingEnd;
        pendingSlack[pendingCount] = slack;
        pendingCount++;
    }

    /**
     * 处理一次 `onUpdateSelection`：是输入法自己某次写入的回声就返回 true。
     *
     * <p>回声按发出的顺序到达，所以对上第 i 条预期时，它之前那些（编辑器没为它们单独回报，比如写完选区没变）也一并作废。对不上的回报是外部变化，以它为准重新定基并清空预期。
     */
    public boolean acknowledge(int newStart, int newEnd, int newComposingStart, int newComposingEnd) {
        int start = BoundsPolicy.atMost(newStart, newEnd);
        int end = BoundsPolicy.atLeast(newStart, newEnd);
        boolean composing = newComposingStart >= 0 && newComposingEnd >= 0;
        int reportedComposingStart = composing ? BoundsPolicy.atMost(newComposingStart, newComposingEnd) : -1;
        int reportedComposingEnd = composing ? BoundsPolicy.atLeast(newComposingStart, newComposingEnd) : -1;
        for (int index = 0; index < pendingCount; index++) {
            int shift = pendingStart[index] - start;
            if (shift < 0 || shift > pendingSlack[index] || pendingEnd[index] - shift != end) continue;
            if (pendingComposingStart[index] < 0 ? composing
                    : pendingComposingStart[index] - shift != reportedComposingStart
                        || pendingComposingEnd[index] - shift != reportedComposingEnd) continue;
            resolve(shift, pendingSlack[index]);
            drop(index + 1);
            return true;
        }
        pendingCount = 0;
        written = false;
        slack = 0;
        known = start >= 0;
        selectionStart = start;
        selectionEnd = end;
        composingStart = reportedComposingStart;
        composingEnd = reportedComposingEnd;
        return false;
    }

    /** 被替换的区间起点：有组字区替换组字区，否则替换选区。 */
    private int replacedStart() {
        return composingStart >= 0 ? composingStart : selectionStart;
    }

    /** 光标左移 `minimum` 到 `maximum` 个单元；有选区、有组字区、可能越过文档开头或不确定量超限时不预测。 */
    private void shiftLeft(int minimum, int maximum) {
        if (!known) return;
        if (selectionStart != selectionEnd || composingStart >= 0
                || selectionStart - slack - maximum < 0 || slack + maximum - minimum > MAX_SLACK) {
            invalidateCurrentPrediction();
            return;
        }
        selectionStart -= minimum;
        selectionEnd = selectionStart;
        slack += maximum - minimum;
        written = true;
    }

    /** This operation cannot be predicted, but earlier writes may still have valid pending echoes. */
    private void invalidateCurrentPrediction() {
        known = false;
        written = false;
        slack = 0;
    }

    /**
     * 回声对上一条记录时不确定量为 `matchedSlack` 的预期，真实位置比它记下的少 `shift`。之后的预期和当前状态都是在它之上继续写出来的：同样减去 `shift`，各自的不确定量只剩之后新增的那部分。
     */
    private void resolve(int shift, int matchedSlack) {
        if (matchedSlack == 0) return;
        for (int index = 0; index < pendingCount; index++) {
            if (pendingSlack[index] < matchedSlack) continue;
            pendingStart[index] -= shift;
            pendingEnd[index] -= shift;
            if (pendingComposingStart[index] >= 0) {
                pendingComposingStart[index] -= shift;
                pendingComposingEnd[index] -= shift;
            }
            pendingSlack[index] -= matchedSlack;
        }
        if (known && slack >= matchedSlack) {
            selectionStart -= shift;
            selectionEnd -= shift;
            if (composingStart >= 0) {
                composingStart -= shift;
                composingEnd -= shift;
            }
            slack -= matchedSlack;
        }
    }

    /** 丢掉最早的 `count` 条预期。 */
    private void drop(int count) {
        int remaining = pendingCount - count;
        System.arraycopy(pendingStart, count, pendingStart, 0, remaining);
        System.arraycopy(pendingEnd, count, pendingEnd, 0, remaining);
        System.arraycopy(pendingComposingStart, count, pendingComposingStart, 0, remaining);
        System.arraycopy(pendingComposingEnd, count, pendingComposingEnd, 0, remaining);
        System.arraycopy(pendingSlack, count, pendingSlack, 0, remaining);
        pendingCount = remaining;
    }
}
