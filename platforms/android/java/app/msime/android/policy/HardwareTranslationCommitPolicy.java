package app.msime.android;

import android.view.KeyEvent;

/**
 * 实体键盘的 Ctrl+Enter 上屏候选译文（#7079），以及它打开的义项页里每个键的去向。
 *
 * <p>桌面三端和鸿蒙在组字时按 Ctrl+Enter 上屏高亮候选的译文：只有一个义项就直接上屏，有几个义项时打开义项页，用 1–9 或空格选。Android 原来只有长按候选这一条路，实体键盘上 Ctrl+Enter 落进「按着修饰键就先上屏原文、再把键交给应用」的兜底分支，结果是拼音上屏、应用收到回车。
 *
 * <p>候选列表在、方案支持插入译文时，这个键总归输入法：没有译文（或释义显示关着）也吃掉、什么也不做，和 Windows、鸿蒙一致，免得聊天应用在拼音还没上屏时就把消息发出去。方案不支持插入译文时（本地模式、日语、韩语，以及不带释义的方案）不接，保持原来的行为。
 *
 * <p>只依赖 {@link KeyEvent} 的常量，结果用 int 常量表示，供 JVM 冒烟直接调用。
 */
public final class HardwareTranslationCommitPolicy {
    /** 不归这条路：交给原来的修饰键分支。 */
    public static final int NONE = 0;
    /** 吃掉按键，什么也不做。 */
    public static final int CONSUME = 1;
    /** 直接上屏唯一的义项。 */
    public static final int COMMIT = 2;
    /** 打开义项页。 */
    public static final int OPEN_PAGE = 3;

    /** 义项页：关页后照常处理这个键。 */
    public static final int PASS = -1;
    /** 义项页：吃掉按键，留在页内。 */
    public static final int IGNORE = -2;
    /** 义项页：只关页，组字保留。 */
    public static final int CLOSE = -3;
    /** 义项页：上屏高亮的义项。 */
    public static final int COMMIT_HIGHLIGHT = -4;
    /** 义项页：高亮移到上一项。 */
    public static final int PREVIOUS = -5;
    /** 义项页：高亮移到下一项。 */
    public static final int NEXT = -6;
    /** 义项页：高亮移到第一项。 */
    public static final int FIRST = -7;
    /** 义项页：高亮移到最后一项。 */
    public static final int LAST = -8;

    private HardwareTranslationCommitPolicy() {}

    /**
     * 只按着 Ctrl 的回车（主键盘或小键盘）该怎么处理。
     *
     * <p>{@code eligible} 是「正在组字、候选非空、当前方案允许插入译文」；释义显示开关不算在里面，开关关着时调用方把 {@code senseCount} 当作 0 传入，结果是吃掉按键。按住不放的自动重复只吃掉，不连续上屏。
     */
    public static int ctrlEnter(int keyCode, boolean shift, boolean ctrl, boolean alt, boolean meta,
                                int repeatCount, boolean eligible, int senseCount) {
        if (!enter(keyCode) || !ctrl || shift || alt || meta || !eligible) return NONE;
        if (repeatCount != 0 || senseCount <= 0) return CONSUME;
        return senseCount == 1 ? COMMIT : OPEN_PAGE;
    }

    /**
     * 屏幕键盘上长按组字中的「确认」键（#6792 第三点）：没有实体键盘的人用它代替 Ctrl+Enter，去向相同，一个义项直接上屏，几个打开义项页（义项页可以点选）。
     *
     * <p>和 Ctrl+Enter 不同，没有译文时返回 {@link #NONE}，长按当作点按：屏幕上的「确认」只确认组字，不会把回车交给应用，没有 Ctrl+Enter 那种把拼音发出去的风险，什么也不做反而让人以为键坏了。长按只有一次，没有自动重复。
     */
    public static int returnHold(boolean eligible, int senseCount) {
        if (!eligible || senseCount <= 0) return NONE;
        return senseCount == 1 ? COMMIT : OPEN_PAGE;
    }

    /** 「确认」键右上角是否标「译」：长按真能上屏译文时才标，标了就和 {@link #returnHold} 的结果一致。 */
    public static boolean marksReturnKey(boolean eligible, int senseCount) {
        return returnHold(eligible, senseCount) != NONE;
    }

    /**
     * 义项页打开时一个按下的键的去向：大于等于 0 是要上屏的义项序号，其余是上面的负数常量。
     *
     * <p>带任何修饰键的组合都是 {@link #PASS}（所以 Ctrl+Enter 会关页后重新开页），单按的修饰键 {@link #IGNORE}，免得按下 Alt 准备下一个组合时页面先没了。会上屏或关页的键在自动重复时 {@link #IGNORE}；方向键可以按住连续移动。
     */
    public static int pageKey(int keyCode, boolean shift, boolean ctrl, boolean alt, boolean meta,
                              int repeatCount, int senseCount) {
        if (modifier(keyCode)) return IGNORE;
        if (shift || ctrl || alt || meta) return PASS;
        int digit = digit(keyCode);
        if (digit > 0) {
            if (repeatCount != 0 || digit > senseCount) return IGNORE;
            return digit - 1;
        }
        if (keyCode == KeyEvent.KEYCODE_SPACE || enter(keyCode))
            return repeatCount != 0 ? IGNORE : COMMIT_HIGHLIGHT;
        if (keyCode == KeyEvent.KEYCODE_ESCAPE || keyCode == KeyEvent.KEYCODE_BACK)
            return repeatCount != 0 ? IGNORE : CLOSE;
        if (keyCode == KeyEvent.KEYCODE_DPAD_UP || keyCode == KeyEvent.KEYCODE_DPAD_LEFT) return PREVIOUS;
        if (keyCode == KeyEvent.KEYCODE_DPAD_DOWN || keyCode == KeyEvent.KEYCODE_DPAD_RIGHT) return NEXT;
        if (keyCode == KeyEvent.KEYCODE_MOVE_HOME) return FIRST;
        if (keyCode == KeyEvent.KEYCODE_MOVE_END) return LAST;
        return PASS;
    }

    /**
     * 义项页把这个结果的键记作「输入法的键」时，它按住的自动重复和随后的抬起都不再交给应用（也不再进 {@link #pageKey}）。上屏、关页和留在页内的键要这样，免得重复的数字或回车落进上屏后已经空闲的输入框；移动高亮的键不记，按住方向键才能连续移动。
     */
    public static boolean holdsKey(int pageKey) {
        return pageKey != PASS && pageKey != PREVIOUS && pageKey != NEXT && pageKey != FIRST && pageKey != LAST;
    }

    /**
     * 云候选、AI 候选异步到达时，runtime 像处理一次输入那样推进 generation，用户却什么也没做。组字和高亮候选都没变时义项页仍然有效，跟到新的 generation 上；任一变了就随旧的 generation 失效。
     */
    public static boolean pageSurvivesRefresh(String openedEditing, String openedCandidate,
                                              String nextEditing, String nextCandidate) {
        return !openedEditing.isEmpty() && openedEditing.equals(nextEditing)
            && !openedCandidate.isEmpty() && openedCandidate.equals(nextCandidate);
    }

    private static boolean enter(int keyCode) {
        return keyCode == KeyEvent.KEYCODE_ENTER || keyCode == KeyEvent.KEYCODE_NUMPAD_ENTER;
    }

    /** 主键盘或小键盘的 1–9，其他键为 0。 */
    private static int digit(int keyCode) {
        if (keyCode >= KeyEvent.KEYCODE_1 && keyCode <= KeyEvent.KEYCODE_9) return keyCode - KeyEvent.KEYCODE_0;
        if (keyCode >= KeyEvent.KEYCODE_NUMPAD_1 && keyCode <= KeyEvent.KEYCODE_NUMPAD_9)
            return keyCode - KeyEvent.KEYCODE_NUMPAD_0;
        return 0;
    }

    private static boolean modifier(int keyCode) {
        return keyCode == KeyEvent.KEYCODE_SHIFT_LEFT || keyCode == KeyEvent.KEYCODE_SHIFT_RIGHT
            || keyCode == KeyEvent.KEYCODE_CTRL_LEFT || keyCode == KeyEvent.KEYCODE_CTRL_RIGHT
            || keyCode == KeyEvent.KEYCODE_ALT_LEFT || keyCode == KeyEvent.KEYCODE_ALT_RIGHT
            || keyCode == KeyEvent.KEYCODE_META_LEFT || keyCode == KeyEvent.KEYCODE_META_RIGHT
            || keyCode == KeyEvent.KEYCODE_FUNCTION;
    }
}
