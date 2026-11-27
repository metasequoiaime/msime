package app.msime.android;

/**
 * 删除键上滑「快速删除」（#5585）：长按会先显示提示，但不改变连删阶段。按住往上滑，离开键的上沿 {@link #REVEAL_DP} dp 后停止连删；手指滑到框的下沿或更高时框变色，表示已经待命，这时松手删掉光标前的全部文字。待命后往下滑回框外即取消待命，再松手什么也不删；进入上滑阶段后，就算滑回键上，这一次按压也不会再连删或删除。
 *
 * <p>坐标都在同一个覆盖层的坐标系里（y 向下增大），单位是像素，`density` 是 dp 到像素的倍数。无 Android 依赖，供 JVM 回归验证。
 */
public final class BackspaceSwipePolicy {
    /** 手指高出删除键上沿这么多才弹出框，免得按键时手指轻微上移就误触。 */
    public static final float REVEAL_DP = 8f;
    /** 待命线通常在键上沿往上这么多：手指要明确地再往上滑一段。 */
    public static final float ARM_DP = 40f;
    /** 框的高度。 */
    public static final float BOX_HEIGHT_DP = 40f;
    /** 键上方空间充足时，框至少保留的高度。 */
    public static final float MIN_BOX_HEIGHT_DP = 28f;
    /** 键上方只剩窄条时仍可显示的紧凑提示框高度。 */
    public static final float COMPACT_BOX_HEIGHT_DP = 16f;
    /** 提示框至少 160 dp 宽，长按时容易辨认，靠边和窄窗口仍限制在键盘内。 */
    public static final float MIN_BOX_WIDTH_DP = 160f;
    /** 框与覆盖层边缘的最小距离。 */
    public static final float MARGIN_DP = 4f;
    /** 待命线至少比弹出线再高这么多：弹出和待命必须是两步，手指弹出框后还要明确地再往上滑一段才会待命。 */
    public static final float MIN_ARM_TRAVEL_DP = 8f;
    /** 紧凑提示框仍须把「显示」和「待命」分成两步。 */
    public static final float COMPACT_MIN_ARM_TRAVEL_DP = 4f;

    /** 这一次按压所处的阶段。 */
    public enum Phase {
        /** 还没弹出框：照常按删除键处理。 */
        IDLE,
        /** 框已弹出但没待命：松手什么也不删。 */
        REVEALED,
        /** 待命：松手删掉光标前的全部文字。 */
        ARMED
    }

    /** 上滑松手这一次允许清除的范围。 */
    public enum ClearTarget { NONE, COMPOSITION, EDITOR }

    /** 一次按压只要碰过组字，就不能在拼音被先前退格删空后再清除已提交正文。 */
    public static ClearTarget clearTarget(Phase phase, boolean touchedComposition, boolean hasComposition) {
        if (!clearsOnRelease(phase)) return ClearTarget.NONE;
        return touchedComposition || hasComposition ? ClearTarget.COMPOSITION : ClearTarget.EDITOR;
    }

    /** 框在覆盖层里的位置。 */
    public record Box(float left, float top, float right, float bottom) {}

    private BackspaceSwipePolicy() { }

    /**
     * 这次按压能不能上滑快速删除：空间充足时按正常框的高度和待命距离判定。九键等布局的删除键贴近覆盖层上沿时，允许使用紧凑框，仍须先越过显示线再越过待命线，避免长按没有任何提示却直接清空。
     *
     * @param keyTop 删除键上沿的 y
     */
    public static boolean available(float keyTop, float density) {
        float normalTop = (MARGIN_DP + MIN_BOX_HEIGHT_DP + REVEAL_DP + MIN_ARM_TRAVEL_DP) * density;
        float compactTop = (MARGIN_DP + COMPACT_BOX_HEIGHT_DP + REVEAL_DP + COMPACT_MIN_ARM_TRAVEL_DP) * density;
        return keyTop >= normalTop || keyTop >= compactTop;
    }

    /**
     * 待命线：手指的 y 小于等于它就待命。空间充足时按正常框计算；紧凑框把待命线放在键上沿上方 12 dp，保留 4 dp 的第二段移动距离。
     *
     * @param keyTop 删除键上沿的 y
     */
    public static float armLine(float keyTop, float density) {
        float normalTop = (MARGIN_DP + MIN_BOX_HEIGHT_DP + REVEAL_DP + MIN_ARM_TRAVEL_DP) * density;
        if (keyTop < normalTop) {
            return keyTop - (REVEAL_DP + COMPACT_MIN_ARM_TRAVEL_DP) * density;
        }
        float preferred = keyTop - ARM_DP * density;
        float lowest = (MARGIN_DP + MIN_BOX_HEIGHT_DP) * density;
        float highestAllowed = keyTop - (REVEAL_DP + MIN_ARM_TRAVEL_DP) * density;
        return BoundsPolicy.bounded(preferred, lowest, highestAllowed);
    }

    /**
     * 框的位置：下沿就是待命线，在删除键上方水平居中（宽度取两个键宽与 {@link #MIN_BOX_WIDTH_DP} 的较大者），整体限制在覆盖层内。紧凑模式的上沿仍贴着覆盖层边缘。
     *
     * @param surfaceWidth 覆盖层宽度
     */
    public static Box box(float keyLeft, float keyTop, float keyRight, float surfaceWidth, float density) {
        float margin = MARGIN_DP * density;
        float bottom = armLine(keyTop, density);
        float top = BoundsPolicy.atLeast(bottom - BOX_HEIGHT_DP * density, margin);
        if (top > bottom) top = bottom;
        float width = BoundsPolicy.atMost(
            BoundsPolicy.atLeast(2 * (keyRight - keyLeft), MIN_BOX_WIDTH_DP * density),
            BoundsPolicy.nonNegative(surfaceWidth - 2 * margin));
        float center = (keyLeft + keyRight) / 2f;
        float left = BoundsPolicy.bounded(center - width / 2f, margin,
            BoundsPolicy.atLeast(surfaceWidth - margin - width, margin));
        return new Box(left, top, left + width, bottom);
    }

    /**
     * 手指移动到 `fingerY` 后的阶段。弹出过的框不会收回：之后只在待命与不待命之间切换。键上方连紧凑框也放不下时始终是 {@link Phase#IDLE}。
     *
     * @param current 移动前的阶段
     * @param fingerY 手指的 y
     * @param keyTop 删除键上沿的 y
     */
    public static Phase next(Phase current, float fingerY, float keyTop, float density) {
        if (!available(keyTop, density)) return Phase.IDLE;
        if (fingerY <= armLine(keyTop, density)) return Phase.ARMED;
        if (current != Phase.IDLE) return Phase.REVEALED;
        return fingerY <= keyTop - REVEAL_DP * density ? Phase.REVEALED : Phase.IDLE;
    }

    /** 松手时是否删掉光标前的全部文字。 */
    public static boolean clearsOnRelease(Phase phase) {
        return phase == Phase.ARMED;
    }

    /** 撤回只在长按后提供；下滑离开键下沿 16 dp 待命，滑回来取消。 */
    public static Phase undoPhase(float fingerY, float keyBottom, float density) {
        return fingerY >= keyBottom + 16f * density ? Phase.ARMED : Phase.REVEALED;
    }

    /**
     * 指尖在删除键布局坐标系里的位置：`fingerY` 是屏幕坐标换到覆盖层坐标系的结果，`keyTop` 是键上沿在同一坐标系里的位置（两者都按此刻的布局读）。
     *
     * <p>按住时键会缩到 0.94（{@code KeyboardPressFeedback}），变换后的上沿比布局上沿低，直接相减会把指尖算得离键下沿更近约 2 dp，下滑撤回在 16 dp 的待命线上就差这一点。按缩放系数换回布局坐标，也就是按下事件里 {@code getY()} 给出的那个位置；没有缩放（1）时不变。
     */
    public static float fingerYInKey(float fingerY, float keyTop, float scale) {
        float offset = fingerY - keyTop;
        return scale > 0f ? offset / scale : offset;
    }

    /** 撤回框放在键下方；最后一排空间不足时贴住键区底部，仍须明确下滑才执行。 */
    public static Box undoBox(float left, float top, float right, float bottom,
                              float width, float height, float density) {
        Box above = box(left, top, right, width, density);
        float y = BoundsPolicy.bounded(bottom + REVEAL_DP * density, MARGIN_DP * density,
            BoundsPolicy.atLeast(MARGIN_DP * density, height - (MARGIN_DP + BOX_HEIGHT_DP) * density));
        return new Box(above.left(), y, above.right(), y + BOX_HEIGHT_DP * density);
    }

    /** 每轮最多读、删的 UTF-16 单元数：编辑器的 getTextBeforeCursor 常常只给一段，大文本要分几轮。 */
    public static final int CLEAR_CHUNK = 4096;
    /** 最多删几轮（约一百万个字符），编辑器不配合时也会停下。 */
    public static final int CLEAR_MAX_ROUNDS = 256;

    /** 删掉光标前文字要用到的两个编辑器操作，对应 InputConnection 的同名方法。 */
    public interface Editor {
        /** 光标前最多 `length` 个 UTF-16 单元；编辑器不支持时为 null。 */
        CharSequence textBeforeCursor(int length);

        /** 删掉光标前 `length` 个 UTF-16 单元；编辑器拒绝时为 false。 */
        boolean deleteBeforeCursor(int length);
    }

    /**
     * 一段一段删掉光标前的全部文字：每轮读光标前至多 {@link #CLEAR_CHUNK} 个单元并按读到的长度整段删掉，读到空、编辑器拒绝删除或满 {@link #CLEAR_MAX_ROUNDS} 轮时停下。按读到的长度删，代理对不会被拆开。不靠「这轮读到的和上一轮一样」判断编辑器没删：一长串重复的文字（满屏的「哈」）每一段本来就一样，那样判断会删到一半就停。默认不保存文字；带 Capture 的重载只在本次编辑器会话内保存有界撤回备份。
     *
     * @return 删掉的 UTF-16 单元数
     */
    public static long clearBeforeCursor(Editor editor) {
        return clearBeforeCursor(editor, null);
    }

    /** 可选备份只收集已接受删除的片段。 */
    public static long clearBeforeCursor(Editor editor, QuickDeleteUndo.Capture capture) {
        long deleted = 0;
        for (int round = 0; round < CLEAR_MAX_ROUNDS; round++) {
            CharSequence before = editor.textBeforeCursor(CLEAR_CHUNK);
            if (before == null || before.length() == 0) break;
            int length = before.length();
            String removed = capture == null ? null : before.toString();
            if (!editor.deleteBeforeCursor(length)) break;
            if (capture != null) capture.deleted(removed);
            deleted += length;
        }
        return deleted;
    }
}
