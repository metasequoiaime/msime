package app.msime.android;

/**
 * 读音行上的组字光标：点读音行把引擎的组字光标移到点中的字母前（#5613），光标不在末尾时把它画进读音行；画出来的是一条皮肤强调色的竖条（#6110，{@code CompositionCaretSpan}），文本里仍是一个 {@link #CARET_MARK}，下标换算不受影响。
 *
 * <p>引擎的光标是 {@code caret_position}，一个落在 {@code editing_text}（打下的原始按键，全拼是字母，九键是数字）里的字节偏移；读音行画的却不一定是这串按键：九键画的是首选读法的拼音（{@code 9426} 画成 {@code xi'an}）或选过拼音后的 {@code ni'426}，里面还夹着引擎自己加的音节分隔。两者按顺序对齐：读音行里只在显示时才有的分隔（{@code '} 和空格）跳过，字母对数字时按九键键位比，其余逐字相同。对不上的读音（读音行显示的不是这串按键）一律不处理，光标照旧在末尾。
 *
 * <p>只有输入法本身让光标留在组字里的方案才可以点：全拼（含九键）、双拼、五笔、粤拼。韩语、注音、越南语、藏文的光标锁在末尾（{@code locks_caret}），移动光标会把组字写进文档；日语和笔画画的是 reading，与按键不是逐字对应。本地模式（表情、计算等）的光标归各模式自己。
 */
public final class CompositionCaretPolicy {
    /** 画在读音行里表示光标的字符。 */
    public static final char CARET_MARK = '|';
    /** 引擎命令：光标左移、右移一格，移到开头、末尾。 */
    public static final int MOVE_LEFT = 4;
    public static final int MOVE_RIGHT = 5;
    public static final int MOVE_HOME = 6;
    public static final int MOVE_END = 7;

    private CompositionCaretPolicy() {}

    /** 这一刻的组字能不能点读音行移光标。 */
    public static boolean editable(int scheme, String localMode, boolean dedicatedEnglish, String editingText) {
        if (dedicatedEnglish || !"none".equals(localMode == null ? "none" : localMode)) return false;
        if (editingText == null || editingText.isEmpty()) return false;
        if (scheme != InputSchemeTraits.QUANPIN && scheme != InputSchemeTraits.SHUANGPIN
                && scheme != InputSchemeTraits.WUBI && scheme != InputSchemeTraits.CANTONESE) return false;
        for (int index = 0; index < editingText.length(); index++) {
            if (editingText.charAt(index) > 0x7f) return false;
        }
        return true;
    }

    /**
     * 读音行文字 {@code display} 的每个字符位置对应的光标位置：{@code result[i]} 是点在 {@code display} 第 i 个字符前面时，光标在 {@code editing} 里的位置；长度是 {@code display.length() + 1}。对不上时为 null。
     */
    static int[] alignment(String display, String editing) {
        if (display == null || editing == null) return null;
        int[] map = new int[display.length() + 1];
        int shown = 0;
        int typed = 0;
        while (shown < display.length()) {
            map[shown] = typed;
            char face = display.charAt(shown);
            if (typed < editing.length() && sameKey(face, editing.charAt(typed))) {
                shown++;
                typed++;
            } else if (separator(face)) {
                shown++;
            } else if (typed < editing.length() && separator(editing.charAt(typed))) {
                typed++;
            } else {
                return null;
            }
        }
        while (typed < editing.length() && separator(editing.charAt(typed))) typed++;
        if (typed != editing.length()) return null;
        map[display.length()] = typed;
        return map;
    }

    /**
     * 点在读音行 {@code tapped} 处（{@code TextView.getOffsetForPosition} 的结果，算的是画出来的文字）时光标该去的位置；对不上时返回 -1。
     *
     * @param prefixLength 画在读音前面、已经选好的那段词的长度：点在它上面就把光标移到开头
     * @param display 读音本身，不含前缀和光标符
     * @param caretMark 画出来的光标符在读音里的位置（插在 {@code display} 的这个下标前），没有画光标时为 -1
     */
    public static int tapTarget(int prefixLength, String display, String editing, int caretMark, int tapped) {
        int[] map = alignment(display, editing);
        if (map == null || tapped < 0) return -1;
        int offset = tapped - BoundsPolicy.nonNegative(prefixLength);
        if (offset <= 0) return 0;
        if (caretMark >= 0 && offset > caretMark) offset--;
        return map[BoundsPolicy.atMost(offset, display.length())];
    }

    /** 光标 {@code caret} 该画在读音的哪个下标前；光标在末尾或对不上时返回 -1。同一个光标位置对应好几处时（中间隔着显示用的分隔）画在最后一处，也就是分隔后面：引擎的退格先删光标前的切分。 */
    public static int markIndex(String display, String editing, int caret) {
        if (editing == null || caret < 0 || caret >= editing.length()) return -1;
        int[] map = alignment(display, editing);
        if (map == null) return -1;
        for (int index = map.length - 1; index >= 0; index--) {
            if (map[index] == caret) return index;
        }
        return -1;
    }

    /** 在 {@code markIndex} 处插入光标符后的读音；不画光标时原样返回。 */
    public static String withMark(String display, int markIndex) {
        if (display == null) return "";
        if (markIndex < 0 || markIndex > display.length()) return display;
        return display.substring(0, markIndex) + CARET_MARK + display.substring(markIndex);
    }

    /**
     * 光标符在读音行整段标题（{@link PhrasePreeditPolicy#title}：已选的词 + 带光标符的读音）里的下标，宿主据此把它画成竖条（#6110）；没有画光标时返回 -1。规则与 {@code title} 相同：本地模式不加前缀。
     *
     * @param caretMark {@link #markIndex} 的结果，光标符插在读音的这个下标前
     */
    public static int markInTitle(String phrasePrefix, int caretMark, boolean localMode) {
        if (caretMark < 0) return -1;
        int prefix = localMode || phrasePrefix == null ? 0 : phrasePrefix.length();
        return prefix + caretMark;
    }

    /**
     * 把光标从 {@code current} 移到 {@code target} 要连发的引擎命令：一个命令码，后面跟着连发几次。移到开头、末尾各一条命令；其余逐格左移或右移。不用移时为空数组。
     */
    public static int[] moves(int current, int target, int length) {
        if (target < 0 || target > length || target == current) return new int[0];
        if (target == 0) return new int[] {MOVE_HOME, 1};
        if (target == length) return new int[] {MOVE_END, 1};
        return target < current
            ? new int[] {MOVE_LEFT, current - target}
            : new int[] {MOVE_RIGHT, target - current};
    }

    static boolean separator(char value) {
        return value == '\'' || value == ' ';
    }

    /** 读音行上的一个字符是不是这个按键：同一个字母（不分大小写），或九键数字键上的字母。 */
    static boolean sameKey(char face, char key) {
        if (Character.toLowerCase(face) == Character.toLowerCase(key)) return true;
        if (key < '2' || key > '9') return false;
        char letter = Character.toLowerCase(face);
        if (letter < 'a' || letter > 'z') return false;
        return KEYPAD.charAt(letter - 'a') == key;
    }

    /** {@code a..z} 各自所在的九键数字键。 */
    private static final String KEYPAD = "22233344455566677778889999";
}
