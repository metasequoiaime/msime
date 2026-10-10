package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 「自动补全成对标点」（共享偏好 `paired_punctuation`）在 Android 上的规则，不依赖 Android，便于在 JVM 上测试。
 *
 * <p>Engine 一次只上屏一个标点，后半个由宿主补：键盘上的标点键沿用 iOS 和 HarmonyOS 宿主的 `PairedPunctuationPolicy`，只在 Engine 的上屏以已知的前半个结尾时补；符号面板里的括号和引号不经过 Engine，由 {@link #symbolClosing} 决定。补上的后半个放在光标右边，光标停在两者之间。
 */
public final class PairedPunctuationPolicy {
    /** Engine 补全一个前半个后，宿主写出的后半个；`opening` 是按下的 ASCII 键，`<` 补完后要通知 Engine 平衡书名号嵌套。 */
    public record Completion(int opening, String closing) {}

    private record EngineCompletion(int opening, String openingMark, String closing) {}

    private static final List<EngineCompletion> ENGINE_COMPLETIONS = List.of(
        new EngineCompletion('"', "“", "”"),
        new EngineCompletion('\'', "‘", "’"),
        new EngineCompletion('(', "（", "）"),
        new EngineCompletion('<', "《", "》"),
        new EngineCompletion('<', "〈", "〉"),
        new EngineCompletion('[', "【", "】"));

    /**
     * 符号面板里点前半个就成对上屏的符号。ASCII 的 `"` 和 `'` 不在里面：它们前后半个是同一个字符，`'` 更常作撇号用，点一下变成两个反而要删。ASCII 的 `<` 也不在里面：它绝大多数时候是小于号，打 `a < b` 时补出的 `>` 只能再删掉；全角 ＜ 照常成对。竖排用的 ︵︶﹁﹂ 也不在里面。
     */
    private static final Map<String, String> SYMBOL_PAIRS = Map.ofEntries(
        Map.entry("（", "）"), Map.entry("《", "》"), Map.entry("〈", "〉"), Map.entry("【", "】"),
        Map.entry("「", "」"), Map.entry("『", "』"), Map.entry("〔", "〕"), Map.entry("〖", "〗"),
        Map.entry("＜", "＞"), Map.entry("｛", "｝"), Map.entry("［", "］"),
        Map.entry("“", "”"), Map.entry("‘", "’"),
        Map.entry("(", ")"), Map.entry("[", "]"), Map.entry("{", "}"));

    private PairedPunctuationPolicy() { }

    /**
     * 打开成对补全时，每按一次引号键都开一对新的。Engine 让引号键在“和”之间交替，是为了没有补全的宿主也能打出后引号；补全的宿主自己写了后半个，从不发出那一次会得到后引号的按键，下一次引号键就会变成孤零零的一个”。所以上屏以后引号结尾时先改回前引号，再补全；关闭补全时保持 Engine 的交替。
     */
    public static String reopenQuote(String commit, int ascii, boolean enabled) {
        if (!enabled || commit == null) return commit;
        if (ascii == '"' && commit.endsWith("”")) return commit.substring(0, commit.length() - 1) + "“";
        if (ascii == '\'' && commit.endsWith("’")) return commit.substring(0, commit.length() - 1) + "‘";
        return commit;
    }

    /** Engine 的上屏以已知的前半个结尾时，要补的后半个；否则为 null。 */
    public static Completion completion(String commit, boolean enabled) {
        if (!enabled || commit == null || commit.isEmpty()) return null;
        for (EngineCompletion entry : ENGINE_COMPLETIONS)
            if (commit.endsWith(entry.openingMark())) return new Completion(entry.opening(), entry.closing());
        return null;
    }

    /**
     * 用 `commitText(closing, 0)` 写完后半个以后，编辑器是不是把光标放到了后半个后面，需要宿主再左移一格。
     *
     * <p>`after` 和 `before` 是写完后光标两侧读到的文字。照规矩处理第二个参数的编辑器里光标后面正是后半个，不动；个别应用（#6458 的 vivo「信息」和系统设置的搜索框）不认不大于 0 的值，把光标放在新文字后面，这时光标后面不是后半个、光标前面正是它。读不出光标后的文字时不知道落在哪，也不动，保持原来的行为。
     */
    public static boolean caretPassedClosing(String closing, CharSequence after, CharSequence before) {
        if (closing == null || closing.isEmpty() || after == null || before == null) return false;
        return !after.toString().startsWith(closing) && before.toString().endsWith(closing);
    }

    /** 符号面板里点 `symbol` 时要一起上屏的后半个；不是成对符号的前半个时为 null。 */
    public static String symbolClosing(String symbol) {
        return symbol == null ? null : SYMBOL_PAIRS.get(symbol);
    }

    /**
     * 键盘补上、后半个还在光标右边等着的那些对，最里层在最后。
     *
     * <p>按下最里层那一对的后半个键时，光标跨过已经在那里的后半个，而不是再写一个：否则（内容 之后再按 ）会得到（内容））；而且每次按引号键都开一对新的（{@link #reopenQuote}），没有这一步就根本打不出后引号。删除、用户移动光标或换了输入框都会清空；一对只在打开它的那个输入框里算数。与 iOS 宿主的 `PairedPunctuationStack` 相同。
     */
    public static final class Stack {
        /** 记得的对数上限，更深的嵌套忘掉最外层。 */
        public static final int LIMIT = 16;
        /** 每个键能跨过的后半个。`<` 视嵌套打开《或〈，所以 `>` 两个都认。 */
        private static final Map<Integer, Set<String>> CLOSINGS = Map.of(
            (int) '"', Set.of("”"), (int) '\'', Set.of("’"), (int) ')', Set.of("）"),
            (int) ']', Set.of("】"), (int) '>', Set.of("》", "〉"));

        private record Entry(String closing, long editor) {}

        private final ArrayList<Entry> entries = new ArrayList<>(LIMIT);

        public boolean isEmpty() { return entries.isEmpty(); }

        /** `editor` 为 0 表示不知道是哪个输入框，不记。 */
        public void push(String closing, long editor) {
            if (editor == 0 || closing == null || closing.isEmpty()) return;
            entries.add(new Entry(closing, editor));
            while (entries.size() > LIMIT) entries.remove(0);
        }

        public void clear() { entries.clear(); }

        /**
         * 按下 `ascii` 时是否跨过最里层那个后半个；是的话把它弹出并返回它，否则返回 null。
         *
         * <p>`following` 是光标后面的文字。编辑器读不出来（null）时只能信这份记录；读出来却不是以那个后半个开头，说明它已经不在了（用户点了别处、应用自己改了文字），记录作废。不收任何后半个的键不动记录：在一对里面打字时光标本来就在后半个前面。
         */
        public String stepOver(int ascii, long editor, CharSequence following) {
            Set<String> candidates = CLOSINGS.get(ascii);
            if (candidates == null || entries.isEmpty()) return null;
            Entry top = entries.get(entries.size() - 1);
            if (!candidates.contains(top.closing()) || top.editor() != editor || editor == 0) {
                clear();
                return null;
            }
            if (following != null && !following.toString().startsWith(top.closing())) {
                clear();
                return null;
            }
            entries.remove(entries.size() - 1);
            return top.closing();
        }

        /**
         * 在符号面板里轻点 `symbol` 时是否跨过最里层那个后半个；是的话把它弹出并返回它，否则返回 null。
         *
         * <p>面板里前半个和后半个挨着摆，点了（ 补成（|）之后再到面板里点 ），应当和按 ) 键一样跨过去，不然会多出一个）。只认和最里层那个后半个完全相同的符号；点了别的后半个说明记录已经对不上，作废；点的不是后半个（在一对里面继续输入）不动记录。`following` 的含义同 {@link #stepOver}。
         */
        public String stepOverSymbol(String symbol, long editor, CharSequence following) {
            if (symbol == null || entries.isEmpty() || !SYMBOL_PAIRS.containsValue(symbol)) return null;
            Entry top = entries.get(entries.size() - 1);
            if (!top.closing().equals(symbol) || top.editor() != editor || editor == 0
                    || (following != null && !following.toString().startsWith(symbol))) {
                clear();
                return null;
            }
            entries.remove(entries.size() - 1);
            return top.closing();
        }
    }
}
