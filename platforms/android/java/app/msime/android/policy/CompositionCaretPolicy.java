package app.msime.android;

/**
 * 读音行上的组字光标：点读音行把引擎的组字光标移到点中的字母前（#5613），光标不在末尾时把它画进读音行；画出来的是一条皮肤强调色的竖条（#6110，{@code CompositionCaretSpan}），文本里仍是一个 {@link #CARET_MARK}，下标换算不受影响。
 *
 * <p>引擎的光标是 {@code caret_position}，一个落在 {@code editing_text}（打下的原始按键，全拼是字母，九键是数字，14 键、18 键是组码字母）里的字节偏移；读音行画的却不一定是这串按键：九键画的是首选读法的拼音（{@code 9426} 画成 {@code xi'an}）或选过拼音后的 {@code ni'426}，14 键把 {@code bugao} 画成 {@code ni'hao}，里面还夹着引擎自己加的音节分隔。两者按顺序对齐：读音行里只在显示时才有的分隔（{@code '} 和空格）跳过，读音的字母按这次组字所在键盘的组码表（{@link #keypad}）对上按键，其余逐字相同。对不上的读音（读音行显示的不是这串按键）一律不处理，光标照旧在末尾。
 *
 * <p>只有输入法本身让光标留在组字里的方案才可以点：全拼（含九键、14 键、18 键）、双拼、五笔、粤拼。韩语、注音、越南语、藏文的光标锁在末尾（{@code locks_caret}），移动光标会把组字写进文档；日语和笔画画的是 reading，与按键不是逐字对应。五笔 14 键的按键是各组的代表字母（QW QW 是 {@code qq}），不是用户打的码，也不能点。本地模式（表情、计算等）的光标归各模式自己。
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

    /** 九键数字键上 {@code a..z} 各自的组码，与引擎 {@code crates/engine/src/nine_key.rs} 的 {@code KEYPAD} 相同。 */
    static final String NINE_KEYPAD = "22233344455566677778889999";
    /** 全拼 14 键上 {@code a..z} 各自的组码（每组首字母，{@code QW} 是 {@code q}、{@code BN} 是 {@code b}），与引擎的 {@code FOURTEEN_KEYPAD} 相同。 */
    static final String FOURTEEN_KEYPAD = "abcdedggujjlmbooqeatucqztz";
    /** 全拼 18 键上 {@code a..z} 各自的组码，与引擎的 {@code EIGHTEEN_KEYPAD} 相同。 */
    static final String EIGHTEEN_KEYPAD = "abxswffhijjlmbipqrsruvwxyz";

    /**
     * 这次组字所在键盘把 {@code a..z} 归成组码的表，第 i 位是第 i 个字母所在键的组码；读音的字母按它对上按键（引擎 {@code KeyGrid::code_of}）。只有全拼的九键、14 键、18 键在网格里组字时有表；26 键（包括网格开着时硬件键盘打的字母，{@code grid_composing} 为假）为 null，只认同一个字母，{@code n} 不会对上 {@code b}。双拼网格（九键、14 键、18 键双拼）一个音节两键、按方案编码，读音的字母与按键不是一一对应，也为 null，读音对不上，照旧退回画原始按键。
     *
     * @param scheme View 的 {@code scheme}
     * @param keyGrid View 的 {@code key_grid}（{@code none}、{@code nine_key}、{@code fourteen_key}、{@code eighteen_key}、{@code nine_key_shuangpin}）
     * @param gridComposing View 的 {@code grid_composing}
     */
    public static String keypad(int scheme, String keyGrid, boolean gridComposing) {
        if (!gridComposing || scheme != InputSchemeTraits.QUANPIN || keyGrid == null) return null;
        return switch (keyGrid) {
            case "nine_key" -> NINE_KEYPAD;
            case "fourteen_key" -> FOURTEEN_KEYPAD;
            case "eighteen_key" -> EIGHTEEN_KEYPAD;
            default -> null;
        };
    }

    /**
     * 这一刻的组字能不能点读音行移光标。
     *
     * @param gridComposing 这次组字在引擎的组码网格里（View 的 {@code grid_composing}）；五笔只有 14 键是网格，这时不能点（#10591，与 iOS 的 {@code typesWubiGrid} 一致）；网格开着时硬件键盘打的五笔码走 26 键，为假，照常可以编辑
     */
    public static boolean editable(int scheme, String localMode, boolean dedicatedEnglish, String editingText,
            boolean gridComposing) {
        if (dedicatedEnglish || !"none".equals(localMode == null ? "none" : localMode)) return false;
        if (editingText == null || editingText.isEmpty()) return false;
        if (scheme != InputSchemeTraits.QUANPIN && scheme != InputSchemeTraits.SHUANGPIN
                && scheme != InputSchemeTraits.WUBI && scheme != InputSchemeTraits.CANTONESE) return false;
        if (scheme == InputSchemeTraits.WUBI && gridComposing) return false;
        for (int index = 0; index < editingText.length(); index++) {
            if (editingText.charAt(index) > 0x7f) return false;
        }
        return true;
    }

    /**
     * 读音行文字 {@code display} 的每个字符位置对应的光标位置：{@code result[i]} 是点在 {@code display} 第 i 个字符前面时，光标在 {@code editing} 里的位置；长度是 {@code display.length() + 1}。对不上时为 null。
     *
     * @param keypad 这次组字所在键盘的组码表（{@link #keypad}），26 键和双拼网格为 null
     */
    static int[] alignment(String display, String editing, String keypad) {
        if (display == null || editing == null) return null;
        int[] map = new int[display.length() + 1];
        int shown = 0;
        int typed = 0;
        while (shown < display.length()) {
            map[shown] = typed;
            char face = display.charAt(shown);
            if (typed < editing.length() && sameKey(face, editing.charAt(typed), keypad)) {
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
     * @param keypad 这次组字所在键盘的组码表（{@link #keypad}）
     */
    public static int tapTarget(int prefixLength, String display, String editing, String keypad, int caretMark,
            int tapped) {
        int[] map = alignment(display, editing, keypad);
        if (map == null || tapped < 0) return -1;
        int offset = tapped - BoundsPolicy.nonNegative(prefixLength);
        if (offset <= 0) return 0;
        if (caretMark >= 0 && offset > caretMark) offset--;
        return map[BoundsPolicy.atMost(offset, display.length())];
    }

    /**
     * 光标 {@code caret} 该画在读音的哪个下标前；光标在末尾或对不上时返回 -1。同一个光标位置对应好几处时（中间隔着只在显示时才有的分隔），按退格真正删掉的那一格来画：
     *
     * <ul>
     *   <li>组码网格（{@code keyGrid}：这次组字是在九键、14 键、18 键、九键双拼的网格里打的，引擎 View 的 {@code grid_composing}）画在最后一处，也就是分隔后面（{@code 94'|26}、14 键的 {@code ba'|ba}）：网格组字的退格先删光标前的用户切分，切分不在 {@code editing_text} 里。读法自带、不是用户切分的分隔（{@code 9426} 读成 {@code xi'an}）也画在后面，那里退格删的是分隔前的数字，两者对不上，#10133 没有改它。</li>
     *   <li>26 键的全拼、双拼、五笔、粤拼画在第一处，也就是分隔前面（{@code zuo|'tian}，#10133）：分隔是引擎加的，不在按键里，退格删的是分隔前的字母；画在分隔后面会让光标从 {@code zuo't|ian} 一步跳到 {@code zuo'|tian} 再跳到 {@code zu|o'tian}，看起来停不到 o 后面。</li>
     * </ul>
     *
     * 不按按键里有没有数字来分：14 键、18 键的按键是组码字母（{@code bugao}），走的却是网格的退格。用户在 26 键自己打的 {@code '} 在按键里，按同一个键一一对齐，不受这条规则影响。
     *
     * @param keypad 这次组字所在键盘的组码表（{@link #keypad}）
     * @param keyGrid 这次组字在引擎的组码网格里（View 的 {@code grid_composing}；网格开着时硬件键盘打的字母走 26 键，为假）
     */
    public static int markIndex(String display, String editing, String keypad, int caret, boolean keyGrid) {
        if (editing == null || caret < 0 || caret >= editing.length()) return -1;
        int[] map = alignment(display, editing, keypad);
        if (map == null) return -1;
        if (keyGrid) {
            for (int index = map.length - 1; index >= 0; index--) {
                if (map[index] == caret) return index;
            }
        } else {
            for (int index = 0; index < map.length; index++) {
                if (map[index] == caret) return index;
            }
        }
        return -1;
    }

    /** 放大条包含末尾光标；原读音行仍只在离开末尾后画光标。 */
    public static int displayOffset(String display, String editing, String keypad, int caret, boolean keyGrid) {
        if (display == null || editing == null || alignment(display, editing, keypad) == null) return -1;
        return caret == editing.length() ? display.length() : markIndex(display, editing, keypad, caret, keyGrid);
    }

    /** 只接住完整组字区内的单点选择，读音分隔符和已选词前缀走同一份映射。 */
    public static int editorTapTarget(String composing, String editing, String keypad, int prefixLength,
            int start, int end, int selectionStart, int selectionEnd) {
        if (composing == null || start < 0 || end - start != composing.length()
                || selectionStart != selectionEnd || selectionStart < start || selectionStart > end
                || prefixLength < 0 || prefixLength > composing.length()) return -1;
        return tapTarget(prefixLength, composing.substring(prefixLength), editing, keypad, -1,
            selectionStart - start);
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

    /** 读音行上的一个字符是不是这个按键：同一个字母（不分大小写），或这个字母在 {@code keypad} 里的组码正是这个按键（九键的数字，14 键、18 键的组码字母）。 */
    static boolean sameKey(char face, char key, String keypad) {
        if (Character.toLowerCase(face) == Character.toLowerCase(key)) return true;
        if (keypad == null) return false;
        char letter = Character.toLowerCase(face);
        if (letter < 'a' || letter > 'z') return false;
        return keypad.charAt(letter - 'a') == key;
    }
}
