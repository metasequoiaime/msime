package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 数字九宫格左列符号栏和右列 ！ 的写法跟着当前语言走。每种方案都借这块九宫格之后（{@link KeyboardLayout#drawsNumberPad}），出厂的数字符号表（{@link NineKeySidebarPolicy#DEFAULT_DIGIT_SYMBOLS}）末尾那几个中文全角标点不能原样出现在英文里：
 *
 * <ul>
 * <li>{@link Style#CHINESE}：中文方案打开中文标点（{@link ChineseSymbolFaces#shouldUseChineseFaces}），符号原样显示、原样上屏，与借用之前的中文数字页相同。</li>
 * <li>{@link Style#JAPANESE}：日文方案（罗马字和假名网格）。逗号写成读点「、」，弯引号写成「」『』，其余全角标点日文同样使用，不变。</li>
 * <li>{@link Style#ASCII}：英文模式、本地模式、写半角 ASCII 标点的方案（韩文、越南语、藏文），以及关掉中文标点的中文方案。中文全角标点换成对应的半角 ASCII，没有对应写法的符号（…、—、·、￥ 等）原样保留。</li>
 * </ul>
 *
 * 用户自定义的数字符号表同样按这条规则换写法：表是给所有九宫格数字键盘共用的，在英文里点到「，」只会写出中文标点。四则运算符号本来就是半角，不受影响。
 *
 * <p>半角写法下，出厂表（共享偏好是空表或与出厂表相同）换完写法后再接上 {@link #ASCII_EXTRAS}：英文、韩文、越南语、藏文这些语言原来的 123 页上有 `; ' " $`，出厂表只有中文用得上的那几个标点，不补的话借来数字九宫格以后这几个就只能去 #+= 或符号面板里找（邮箱框里的 `@` 本来就在表里）。用户自定义的表不接：那是用户自己挑的符号和顺序，只照上面的规则换写法，不往里加东西。中文和日文写法不接。接上的符号和左列其他出厂项一样上屏：通常按字面，本地模式、越南语和藏文里 Engine 当作拼写的交给会话（{@link #typesThroughSession}）。这条规则与 iOS、HarmonyOS 逐项相同。
 */
public final class NumberPadPunctuation {
    /** 数字九宫格此刻的标点写法。 */
    public enum Style { CHINESE, JAPANESE, ASCII }

    private static final Map<String, String> ASCII = Map.ofEntries(
        Map.entry("，", ","), Map.entry("。", "."), Map.entry("？", "?"), Map.entry("！", "!"),
        Map.entry("、", ","), Map.entry("；", ";"), Map.entry("：", ":"),
        Map.entry("（", "("), Map.entry("）", ")"), Map.entry("（）", "()"),
        Map.entry("“", "\""), Map.entry("”", "\""), Map.entry("‘", "'"), Map.entry("’", "'"),
        Map.entry("《", "<"), Map.entry("》", ">"), Map.entry("【", "["), Map.entry("】", "]"),
        Map.entry("～", "~"));

    private static final Map<String, String> JAPANESE = Map.ofEntries(
        Map.entry("，", "、"),
        Map.entry("“", "「"), Map.entry("”", "」"), Map.entry("‘", "『"), Map.entry("’", "』"));

    private NumberPadPunctuation() {}

    /**
     * 按当前方案和模式取写法：中文标点生效时是中文（与 123 层、键面提示同一个判断）；日文方案在没有英文模式、没有本地模式时是日文；其余都是半角 ASCII。
     *
     * @param chinesePunctuation 用户的中文标点开关
     * @param dedicatedEnglish 宿主处在英文模式
     * @param scheme View 报告的方案编号，未知时为 -1
     * @param localMode View 的 `local_mode`，没有本地模式时是 `none`
     */
    public static Style style(boolean chinesePunctuation, boolean dedicatedEnglish, int scheme, String localMode) {
        if (ChineseSymbolFaces.shouldUseChineseFaces(dedicatedEnglish, scheme, localMode, chinesePunctuation))
            return Style.CHINESE;
        if (!dedicatedEnglish && scheme == InputSchemeTraits.JAPANESE && "none".equals(localMode))
            return Style.JAPANESE;
        return Style.ASCII;
    }

    /** 一个符号在这种写法下的样子；表里没有的符号原样返回。 */
    public static String face(String symbol, Style style) {
        if (symbol == null) return "";
        return switch (style) {
            case CHINESE -> symbol;
            case JAPANESE -> JAPANESE.getOrDefault(symbol, symbol);
            case ASCII -> ASCII.getOrDefault(symbol, symbol);
        };
    }

    /** 整张符号表换成这种写法；换完撞上前面已有的同一个符号（例如「，」和「、」在 ASCII 下都是 `,`）时只留第一个，免得符号栏里出现两个一样的键。 */
    public static List<String> symbols(List<String> symbols, Style style) {
        if (style == Style.CHINESE) return symbols;
        List<String> result = new ArrayList<>(symbols.size());
        for (String symbol : symbols) {
            String localized = face(symbol, style);
            if (!result.contains(localized)) result.add(localized);
        }
        return List.copyOf(result);
    }

    /** 半角写法下接在出厂表后面的半角符号，按这个顺序，表里已经有的跳过。 */
    public static final List<String> ASCII_EXTRAS = List.of("@", ":", ";", "(", ")", "'", "\"", "$");

    /**
     * 数字九宫格左列此刻显示的符号：先按 {@link #symbols(List, Style)} 换写法；半角写法且用的是出厂表时，再按 {@link #ASCII_EXTRAS} 的顺序接上表里还没有的符号。出厂数字表在半角下是 `+ - * / = % ( ) : @ , . ? ! ; ' " $`。
     *
     * @param symbols 这一刻的数字表（{@link NineKeySidebarPolicy#digitSymbols}：空表已经换成出厂表）
     * @param custom 共享偏好里的表是用户自定义的（{@link NineKeySidebarPolicy#custom}）；自定义的表只换写法，不接
     */
    public static List<String> symbols(List<String> symbols, Style style, boolean custom) {
        List<String> localized = symbols(symbols, style);
        if (style != Style.ASCII || custom) return localized;
        List<String> result = new ArrayList<>(localized);
        for (String extra : ASCII_EXTRAS)
            if (!result.contains(extra)) result.add(extra);
        return List.copyOf(result);
    }

    /**
     * 数字九宫格上的一个键要不要先作为字符交给会话，而不是按字面上屏（先结束组字再写出这个键）。适用于数字、0、小数点、左列出厂表的项（运算符和半角写法接上的 {@link #ASCII_EXTRAS}）和第五排补充行的键；用户自定义的左列表项和右列的 ！ 不走这里，总是按字面上屏。与 iOS `typeNumberPadText`、HarmonyOS 的规则相同：
     *
     * <ul>
     * <li>只在本地模式和越南语、藏文（字母按大小写拼写的方案）里才看：中文各方案、英文、日文、韩文和大千注音一律按字面上屏。中文拼写里的 `$ %`、`www` 后的 `.` 在数字九宫格上仍按字面；大千把 1–0 列为拼写符号是因为它的数字行就是注音键，数字九宫格上的数字只是数字。</li>
     * <li>藏文的 `'`（achung，如 `'od`）总是先交给会话：没有组字时它也能起头一个音节，Engine 空闲时不一定把它算作「组字中的拼写」。</li>
     * <li>其余的键在 Engine 此刻把它列为拼写符号时交给会话：本地模式里（`local_mode` 不是 `none`，算作组字中）或有组字时，`View.spelling_symbols` 列着这个字符（{@link NumberRowSelectionPolicy#engineSpells}，硬件数字行用的同一条判断）。这样 U 模式的十六进制数字、计算和网址模式的数字与符号、VNI 越南语正在拼的词的声调数字、藏文正在拼时的 `+ - . /` 都留在组字里，不因为 123 换成了数字九宫格就被截断。</li>
     * </ul>
     *
     * 会话不收（返回未处理）时调用方照旧按字面上屏。
     *
     * @param symbol 这个键写出的文字；不是单个 ASCII 可见字符时一律按字面
     * @param localMode View 的 `local_mode`，没有本地模式时是 `none`
     * @param casedLetters 越南语或藏文方案生效（不在英文模式、不在本地模式），{@code MSIMEInputService.letterCaseSchemeActive}
     * @param tibetan 藏文方案生效，{@code MSIMEInputService.tibetanSchemeActive}
     * @param editingText View 的 `editing_text`
     * @param spellingSymbols View 的 `spelling_symbols`
     */
    public static boolean typesThroughSession(String symbol, String localMode, boolean casedLetters, boolean tibetan,
            String editingText, String spellingSymbols) {
        if (symbol == null || symbol.length() != 1) return false;
        boolean local = localMode != null && !"none".equals(localMode);
        if (!local && !casedLetters) return false;
        if (tibetan && "'".equals(symbol)) return true;
        return NumberRowSelectionPolicy.engineSpells(localMode, editingText, spellingSymbols, symbol.charAt(0));
    }

    /** 数字九宫格那一页第五排的括号用不用全角：中文和日文用全角，其余半角（{@link KeyboardLayout#symbolExtraRow}）。 */
    public static boolean fullWidthBrackets(Style style) {
        return style != Style.ASCII;
    }
}
