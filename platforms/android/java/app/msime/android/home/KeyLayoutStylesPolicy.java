package app.msime.android.home;

import app.msime.android.EighteenKeyLayout;
import app.msime.android.FourteenKeyLayout;
import app.msime.android.KeyHeatmapLayout;
import app.msime.android.KeyPressIds;
import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.KeyboardScheme;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.TouchKeyStyle;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * 「按键布局」子页（{@link KeyLayoutStylesPage}）里不碰 Android 的部分：列出哪几种键盘类型、默认停在哪一种、每种样式叫什么、选中后下面那行说明写什么、预览高亮哪几个键，整块键盘预览画哪些键，以及两行左侧符号（#10223）的标题、摘要和编辑页说明。三端（iOS、共享设置页）按同一份规格画同样的页面。
 *
 * <p>预览的键序、宽度份额和跨行取自键盘自己用的表：26 键和九键外框取统计页热力图的同一套布局（{@link KeyHeatmapLayout#soft}、{@link KeyHeatmapLayout#nine}，由 KeyHeatmapLayoutSmoke 钉住与键盘一致），14 键与 18 键取 {@link FourteenKeyLayout}、{@link EighteenKeyLayout} 和 `ImeLetterRows#rebuildFourteenKeyRows` 用的份额，底行都是 {@link KeyboardActionRow#designEntries}。check-host 直接编译并在 JVM 里测它。
 *
 * <p>皮肤页那几块整键盘预览（{@link KeyboardPreview}）也从这里取行（{@link Preview}），所以它们和这一页、和键盘本身画的是同一副底行。
 */
public final class KeyLayoutStylesPolicy {
    private KeyLayoutStylesPolicy() {}

    /** 页面顶部分段控件里的一种键盘类型，各自读写一项共享偏好。 */
    public enum Kind {
        TWENTY_SIX("26 键", TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE, TouchKeyStyle.TWENTY_SIX_KEY_ROWS),
        FOURTEEN("14/18 键", TouchKeyStyle.FOURTEEN_KEY_PREFERENCE, TouchKeyStyle.FOURTEEN_KEY_ROWS),
        NINE("9 键", TouchKeyStyle.NINE_KEY_PREFERENCE, TouchKeyStyle.NINE_KEY_ROWS);

        /** 分段控件上的字。 */
        public final String title;
        /** 这一类样式存在哪一项共享偏好里。 */
        public final String preference;
        /** 这一类提供的样式，按胶囊的排列顺序。 */
        public final List<TouchKeyStyle.Row> rows;

        Kind(String title, String preference, List<TouchKeyStyle.Row> rows) {
            this.title = title;
            this.preference = preference;
            this.rows = rows;
        }

        /** 用户给这一类选的样式。 */
        public TouchKeyStyle.Row selected(TouchKeyStyle.Styles styles) {
            return switch (this) {
                case TWENTY_SIX -> styles.twentySix();
                case FOURTEEN -> styles.fourteen();
                case NINE -> styles.nine();
            };
        }

        /** 只把这一类换成 `row`，另外两类不动；页面点了胶囊后先用它就地换预览，再写偏好。 */
        public TouchKeyStyle.Styles with(TouchKeyStyle.Styles styles, TouchKeyStyle.Row row) {
            return switch (this) {
                case TWENTY_SIX -> new TouchKeyStyle.Styles(row, styles.fourteen(), styles.nine());
                case FOURTEEN -> new TouchKeyStyle.Styles(styles.twentySix(), row, styles.nine());
                case NINE -> new TouchKeyStyle.Styles(styles.twentySix(), styles.fourteen(), row);
            };
        }
    }

    /** 本版本提供的键盘类型，按 26 键、14/18 键、9 键排列（{@link TouchKeyStyle.Groups}，键盘页入口按同一条规则显示）；只有一种时页面不画分段控件。 */
    public static List<Kind> offered(TouchKeyStyle.Groups groups) {
        List<Kind> kinds = new ArrayList<>(3);
        if (groups.twentySix()) kinds.add(Kind.TWENTY_SIX);
        if (groups.fourteen()) kinds.add(Kind.FOURTEEN);
        if (groups.nine()) kinds.add(Kind.NINE);
        return List.copyOf(kinds);
    }

    /**
     * 页面打开时停在哪一种：用户当前在用的中文键盘类型。全拼、双拼、五笔、粤拼的 26 键是 26 键；全拼 14 键与 18 键、双拼和五笔选了 14 键或 18 键排法是 14/18 键；全拼九键、九键双拼和笔画是 9 键。判断不了（手写、其他语言）或本版本不提供那一类时停在第一个；本版本一类都没有时为 null。
     *
     * @param current 共享偏好里存的入口（{@code SchemePreferences.storedScheme}，键盘页「中文键盘」读的同一个）
     * @param shuangpinLayout 双拼的排法 `touch_shuangpin_layout`
     * @param wubiLayout 五笔的排法 `touch_wubi_layout`
     */
    public static Kind initialKind(List<Kind> offered, KeyboardScheme current, String shuangpinLayout, String wubiLayout) {
        if (offered.isEmpty()) return null;
        Kind kind = kindOf(current, shuangpinLayout, wubiLayout);
        return kind != null && offered.contains(kind) ? kind : offered.get(0);
    }

    private static Kind kindOf(KeyboardScheme current, String shuangpinLayout, String wubiLayout) {
        if (current == null) return null;
        if (current.shuangpinProfile() != null)
            return fourteenKeyLayout(shuangpinLayout) ? Kind.FOURTEEN : Kind.TWENTY_SIX;
        return switch (current) {
            case QUANPIN, CANTONESE, CANTONESE_SHUANGPIN -> Kind.TWENTY_SIX;
            case WUBI -> "fourteen_key".equals(wubiLayout) ? Kind.FOURTEEN : Kind.TWENTY_SIX;
            case QUANPIN_FOURTEEN_KEY, QUANPIN_EIGHTEEN_KEY -> Kind.FOURTEEN;
            case QUANPIN_NINE_KEY, NINE_KEY_SHUANGPIN, STROKE -> Kind.NINE;
            default -> null;
        };
    }

    private static boolean fourteenKeyLayout(String layout) {
        return "fourteen_key".equals(layout) || "eighteen_key".equals(layout);
    }

    /** 14/18 键的预览画 18 键的字母格：用户此刻在用全拼 18 键或双拼的 18 键排法；其余时候画 14 键（五笔没有 18 键）。 */
    public static boolean eighteen(KeyboardScheme current, String shuangpinLayout) {
        if (current == null) return false;
        if (current.shuangpinProfile() != null) return "eighteen_key".equals(shuangpinLayout);
        return current == KeyboardScheme.QUANPIN_EIGHTEEN_KEY;
    }

    /** 胶囊上的名字；26 键与 14/18 键的逗号句号样式叫「逗号句号」，九键的叫「底栏带标点」，因为九键的标点原本在侧栏里；「右侧大回车」只有九键有。 */
    public static String label(Kind kind, TouchKeyStyle.Row row) {
        return switch (row) {
            case STANDARD -> "标准";
            case COMMA_PERIOD -> kind == Kind.NINE ? "底栏带标点" : "逗号句号";
            case LONG_SPACE -> "长空格";
            case EMOJI -> "表情键";
            case TALL_RETURN -> "右侧大回车";
        };
    }

    // 选中样式的副作用，写在胶囊下面一行；三端逐字相同。
    static final String LONG_SPACE_NOTE = "符号键移到工具栏。";
    static final String EMOJI_NOTE = "长按 123 打开符号。";
    static final String TALL_RETURN_NOTE = "回车竖占两行，符号键在左下角。";

    // 两行左侧符号（9 键分段卡片里一行、「数字键盘」一组后一行）的标题和编辑页顶部的说明，三端逐字相同（iOS `NineKeySidebarPreference`、共享设置页 `touch-key-layout-styles.ts`），scripts/test-nine-key-sidebar-parity.py 核对。
    public static final String NINE_KEY_SIDEBAR_TITLE = "九键左侧符号";
    public static final String DIGIT_SIDEBAR_TITLE = "数字键盘左侧符号";
    public static final String NINE_KEY_SIDEBAR_NOTE = "拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。";
    public static final String DIGIT_SIDEBAR_NOTE = "所有九宫格数字键盘共用，#+= 固定在最后。";
    /** 导航行右侧的摘要最多列几项，多出来的写成「等 N 个」。 */
    static final int SIDEBAR_SUMMARY_LIMIT = 4;

    /** 导航行右侧的摘要：存的表（空表时是本端出厂表 `defaults`）不超过四项时用空格连起来，多于四项时前四项加「 等 N 个」。 */
    public static String sidebarSummary(List<String> stored, List<String> defaults) {
        return NineKeySidebarPolicy.summary(NineKeySidebarPolicy.symbols(stored, defaults), SIDEBAR_SUMMARY_LIMIT);
    }

    /** 胶囊下面那行说明，只说当前选中样式不显而易见的副作用；没有副作用的样式为 null，页面不画这一行。 */
    public static String note(Kind kind, TouchKeyStyle.Row row) {
        return switch (row) {
            case LONG_SPACE -> kind == Kind.TWENTY_SIX ? LONG_SPACE_NOTE : null;
            case EMOJI -> kind == Kind.NINE ? null : EMOJI_NOTE;
            case TALL_RETURN -> kind == Kind.NINE ? TALL_RETURN_NOTE : null;
            default -> null;
        };
    }

    /**
     * 预览里用强调色描边的键：这个样式与「标准」不同的那几个，按键的 id（{@link Key#id}）给出，不比较键面文字。标准样式不高亮。
     *
     * <p>26 键：逗号句号在空格两侧高亮 ， 。，长空格高亮空格，表情键高亮 😀。14/18 键：长空格高亮空格，表情键高亮 😀。9 键：底栏带标点高亮 ， 。，表情键高亮 😀，右侧大回车高亮回车。
     */
    public static Set<String> highlights(Kind kind, TouchKeyStyle.Row row) {
        if (!kind.rows.contains(row)) return Set.of();
        return switch (row) {
            case STANDARD -> Set.of();
            case COMMA_PERIOD -> Set.of(COMMA, PERIOD);
            case LONG_SPACE -> Set.of(SPACE);
            case EMOJI -> Set.of(EMOJI);
            case TALL_RETURN -> Set.of(RETURN);
        };
    }

    /** 同上，另按九键左侧符号（`touch_nine_key_sidebar_symbols`）：9 键分段里字母表是自定义的（{@link NineKeySidebarPolicy#custom}）时，再高亮整条左列（{@link #SIDEBAR}）；空表或与出厂表相同时不高亮。 */
    public static Set<String> highlights(Kind kind, TouchKeyStyle.Row row, boolean symbolFirst, List<String> nineKeySidebar) {
        Set<String> base = highlights(kind, row, symbolFirst);
        if (kind != Kind.NINE || !NineKeySidebarPolicy.custom(nineKeySidebar, NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS))
            return base;
        Set<String> marked = new HashSet<>(base);
        marked.add(SIDEBAR);
        return Set.copyOf(marked);
    }

    /** 同上，另按「26 键「符」与「123」互换」（`touch_twenty_six_symbol_first`）：打开时 26 键所选样式的底行真有 符 可换（{@link KeyboardActionRow#symbolFirstMoves}，标准和逗号句号；长空格、表情键没有 符），在样式的高亮之外再高亮 符 和 123。 */
    public static Set<String> highlights(Kind kind, TouchKeyStyle.Row row, boolean symbolFirst) {
        Set<String> style = highlights(kind, row);
        if (!symbolFirst || kind != Kind.TWENTY_SIX
                || !KeyboardActionRow.symbolFirstMoves(KeyboardLayout.STANDARD_TOUCH_LAYOUT, offered(kind, row)))
            return style;
        Set<String> swapped = new HashSet<>(style);
        swapped.add(SYMBOL);
        swapped.add(LAYER);
        return Set.copyOf(swapped);
    }

    /** 同上，另按「中/英键放在空格左边」（`touch_language_key_side` 为 `left`）：打开时在样式和互换的高亮之外再高亮中/英这一个键；被它挤动的空格和标点宽度、样子都没变，不高亮。三类键面的预览底行都有中/英（右侧大回车在九宫格下方那一行），所以三类都高亮。 */
    public static Set<String> highlights(Kind kind, TouchKeyStyle.Row row, boolean symbolFirst, boolean languageLeft) {
        Set<String> base = highlights(kind, row, symbolFirst);
        if (!languageLeft) return base;
        Set<String> moved = new HashSet<>(base);
        moved.add(LANGUAGE);
        return Set.copyOf(moved);
    }

    /** 同上，「中/英键放在空格左边」与九键左侧符号的高亮都加上：两个重载各管一项，互不影响。 */
    public static Set<String> highlights(Kind kind, TouchKeyStyle.Row row, boolean symbolFirst, boolean languageLeft,
            List<String> nineKeySidebar) {
        Set<String> base = highlights(kind, row, symbolFirst, nineKeySidebar);
        if (!languageLeft) return base;
        Set<String> moved = new HashSet<>(base);
        moved.add(LANGUAGE);
        return Set.copyOf(moved);
    }

    /** 预览画不画顶部数字行：开关打开、画的是 26 键或 14/18 键时才画；9 键不画，开关只对 26/14/18 键生效。 */
    public static boolean drawsNumberRow(Kind kind, boolean numberRow) {
        return numberRow && kind != Kind.NINE;
    }

    // 预览里几个键的 id，与按键热力图记的相同（KeyHeatmapLayout）。
    static final String COMMA = "Comma";
    static final String PERIOD = "Period";
    static final String SPACE = "Space";
    static final String EMOJI = "SoftEmoji";
    static final String RETURN = "Enter";
    static final String SYMBOL = "SoftSymbol";
    static final String LAYER = "SoftLayer";
    /** 9 键预览里画成一整条的左侧符号栏。键盘上这一列记 `SoftPunctuation`，可标准外框右列的 ！ 也记它，按它高亮会连 ！ 一起描边，所以预览给整条左列单起一个只在预览里用的 id。 */
    static final String SIDEBAR = "NineKeySidebar";
    static final String LANGUAGE = "SoftLanguage";
    private static final String SHIFT = "ShiftLeft";
    /** 热力图把 26 键第三行左端画成组字中的「分词」、记 `Quote`；预览画空闲时的 ⇧。 */
    private static final String SEPARATOR = "Quote";
    private static final Set<String> FUNCTION_IDS = Set.of(SHIFT, SEPARATOR, "Backspace", SYMBOL, LAYER,
        LANGUAGE, "SoftGlobe", EMOJI);

    /** 预览里一个键的底色：字符键用键帽色，功能键用功能键色，回车用强调色。 */
    public enum Tone { CHARACTER, FUNCTION, RETURN }

    /**
     * 预览里的一个键：键面、在这一行里的宽度份额、id（没有时为空字符串）、底色，以及占几行（2、3 表示跨到下面几行，0 表示被上一行跨下来的键盖住或只是占位的缩进，不画、只占宽度）。`stack` 不为空时这一格是一条竖栏：把占的整块高度等分给 `stack` 里的每一项，各画成一个键，`face` 不画（9 键的左侧符号栏）。
     */
    public record Key(String face, float weight, String id, Tone tone, int rows, List<String> stack) {
        public Key {
            stack = List.copyOf(stack);
        }

        public Key(String face, float weight, String id, Tone tone, int rows) {
            this(face, weight, id, tone, rows, List.of());
        }
    }

    /**
     * 整块键盘预览，自上而下逐行，每行按份额铺满整行：
     *
     * <ul>
     * <li>26 键：顶部数字行（{@link #drawsNumberRow}）、三行字母（第三行两端 ⇧ 和 ⌫）和所选样式的底行。</li>
     * <li>14/18 键：顶部数字行、左侧符号栏连同三行字母格（18 键第二行两端各缩进半键，第三行两端是 符 和 ⌫），以及所选样式的底行。符号栏和字母列按键盘的 {@link FourteenKeyLayout#SPELLING_SIDEBAR_WEIGHT} : {@link FourteenKeyLayout#LETTER_COLUMN_WEIGHT} 并排，栏里逐行画九键默认的三个符号。</li>
     * <li>9 键：左侧符号栏、3×3 九宫格和右列，以及所选样式的底行；右侧大回车是回车从第三行跨到底行、符 在符号栏下方的那一整块外框。</li>
     * </ul>
     */
    public static List<List<Key>> keyboard(Kind kind, TouchKeyStyle.Row style, boolean numberRow, boolean eighteen) {
        return keyboard(kind, style, numberRow, eighteen, false);
    }

    /** 同上，`symbolFirst`（「26 键「符」与「123」互换」）打开时 26 键底行按键盘的同一条规则把 符 排到 123 左边（{@link KeyHeatmapLayout#soft(TouchKeyStyle.Row, boolean, boolean)}），没有 符 的样式不变；14/18 键和 9 键不受它影响。 */
    public static List<List<Key>> keyboard(Kind kind, TouchKeyStyle.Row style, boolean numberRow, boolean eighteen,
            boolean symbolFirst) {
        return keyboard(kind, style, numberRow, eighteen, symbolFirst, false, List.of());
    }

    /** 同上，`languageLeft`（「中/英键放在空格左边」）打开时三类键面的底行都按键盘的同一条规则把中/英排到空格左边（{@link KeyboardActionRow#languageBeforeSpace}），排在「符」与「123」互换之后；右侧大回车是九宫格下方那一行。 */
    public static List<List<Key>> keyboard(Kind kind, TouchKeyStyle.Row style, boolean numberRow, boolean eighteen,
            boolean symbolFirst, boolean languageLeft) {
        return keyboard(kind, style, numberRow, eighteen, symbolFirst, languageLeft, List.of());
    }

    /** 同上，中/英在右边，只换九键左侧符号栏的表，见下一个重载。 */
    public static List<List<Key>> keyboard(Kind kind, TouchKeyStyle.Row style, boolean numberRow, boolean eighteen,
            boolean symbolFirst, List<String> nineKeySidebar) {
        return keyboard(kind, style, numberRow, eighteen, symbolFirst, false, nineKeySidebar);
    }

    /**
     * 同上，「中/英键放在空格左边」之外，9 键的左侧符号栏按共享偏好 `touch_nine_key_sidebar_symbols` 画，与键盘相同（{@link NineKeySidebarPolicy#letterSymbols(List, boolean)}）：自定义的表画一屏，最多 {@link NineKeySidebarPolicy#VISIBLE_ROWS} 项，等高均分整条左列；空表画出厂的 ， 。 ？，右侧大回车时末尾接 ！ 共四项。只有出厂表、又不是右侧大回车时仍按热力图逐行画三格（与 #10208 的画面相同）。14/18 键和 26 键不受符号表影响；左列只换前三行的第一格，底行中/英的位置不受符号表影响。
     */
    public static List<List<Key>> keyboard(Kind kind, TouchKeyStyle.Row style, boolean numberRow, boolean eighteen,
            boolean symbolFirst, boolean languageLeft, List<String> nineKeySidebar) {
        List<List<Key>> rows = keyboardRows(kind, style, numberRow, eighteen, symbolFirst, languageLeft);
        if (kind != Kind.NINE) return rows;
        boolean tallReturn = offered(kind, style) == TouchKeyStyle.Row.TALL_RETURN;
        if (!tallReturn && !NineKeySidebarPolicy.custom(nineKeySidebar, NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS))
            return rows;
        return withSidebar(rows, NineKeySidebarPolicy.letterSymbols(nineKeySidebar, tallReturn));
    }

    /** 9 键预览前三行的第一格换成一整条左侧符号栏：第一行那一格跨三行、带着一屏的符号，下面两行同一格只占宽度。 */
    private static List<List<Key>> withSidebar(List<List<Key>> rows, List<String> symbols) {
        List<String> visible = symbols.subList(0, Math.min(symbols.size(), NineKeySidebarPolicy.VISIBLE_ROWS));
        int spanned = Math.min(3, rows.size() - 1);
        List<List<Key>> result = new ArrayList<>(rows.size());
        for (int index = 0; index < rows.size(); index++) {
            List<Key> row = rows.get(index);
            if (index >= spanned) {
                result.add(row);
                continue;
            }
            List<Key> replaced = new ArrayList<>(row);
            float weight = row.get(0).weight();
            replaced.set(0, index == 0 ? new Key("", weight, SIDEBAR, Tone.CHARACTER, spanned, visible)
                : new Key("", weight, "", Tone.CHARACTER, 0));
            result.add(List.copyOf(replaced));
        }
        return List.copyOf(result);
    }

    private static List<List<Key>> keyboardRows(Kind kind, TouchKeyStyle.Row style, boolean numberRow, boolean eighteen,
            boolean symbolFirst, boolean languageLeft) {
        boolean digits = drawsNumberRow(kind, numberRow);
        return switch (kind) {
            case TWENTY_SIX -> converted(KeyHeatmapLayout.soft(offered(kind, style), digits, symbolFirst, languageLeft));
            case NINE -> nineActionColumn(converted(KeyHeatmapLayout.nine(offered(kind, style), languageLeft)));
            case FOURTEEN -> fourteen(offered(kind, style), digits, eighteen, languageLeft);
        };
    }

    /** 九键右列（⌫、重输，以及标准外框最下面的 ！）在键盘上都是功能键（`ImeLayoutRows` 给它们 `KeyboardKeyRole.ACCENT`）；！ 与左栏的符号同记 `SoftPunctuation`，按 id 分不出来，这里按位置把右列改成功能键，回车不变。 */
    private static List<List<Key>> nineActionColumn(List<List<Key>> rows) {
        List<List<Key>> result = new ArrayList<>(rows.size());
        for (int index = 0; index < rows.size(); index++) {
            List<Key> row = rows.get(index);
            Key right = row.get(row.size() - 1);
            if (index == rows.size() - 1 || right.tone() != Tone.CHARACTER) {
                result.add(row);
                continue;
            }
            List<Key> changed = new ArrayList<>(row);
            changed.set(row.size() - 1, new Key(right.face(), right.weight(), right.id(), Tone.FUNCTION, right.rows()));
            result.add(List.copyOf(changed));
        }
        return List.copyOf(result);
    }

    /**
     * 「按键布局」页以外那几块整键盘预览（AI 皮肤页、皮肤列表、社区皮肤卡片和详情，{@link KeyboardPreview}）画哪一副：用户自己的共享偏好里这一类键盘选的样式、顶部数字行、「符」与「123」互换、九键左侧符号（`touch_nine_key_sidebar_symbols`）、「中/英键放在空格左边」（`touch_language_key_side`），以及本机的「交换语言键与数字键」（`platform.android.swap_language_number_keys`）。键序和份额就是 {@link #keyboard} 那一副，不做高亮。Android 没有隐藏中/英键的设置，底行总画中/英。
     *
     * @param kind 画哪一类键盘（{@link #previewKind}，AI 皮肤页由它自己的「26 键 | 9 键」分段定）
     * @param eighteen 14/18 键画 18 键的字母格（{@link #eighteen}）
     * @param swapLanguageNumber 本机设置「交换语言键与数字键」：底行的 123 与中/英对调，两键的宽度跟着键走，与键盘的 {@code KeyboardActionRow} 相同
     * @param nineKeySidebar 九键字母层左侧符号栏的表（空表是出厂表），9 键时左列与「按键布局」页、键盘画同一屏
     * @param languageLeft 「中/英键放在空格左边」：三类键盘的底行都先把中/英排到空格左边，再做本机的交换，与键盘的次序相同
     */
    public record Preview(Kind kind, TouchKeyStyle.Styles styles, boolean numberRow, boolean eighteen,
            boolean symbolFirst, boolean swapLanguageNumber, List<String> nineKeySidebar, boolean languageLeft) {
        /** 拿不到偏好时（读偏好失败、尚未读完，或展示的东西根本不画预览）画的那一副：26 键标准底行，不开数字行、不互换。 */
        public static final Preview STANDARD = new Preview(Kind.TWENTY_SIX, TouchKeyStyle.Styles.DEFAULT, false, false,
            false, false);

        public Preview {
            if (kind == null || styles == null) throw new IllegalArgumentException("Preview needs a kind and styles");
            nineKeySidebar = List.copyOf(nineKeySidebar);
        }

        /** 九键左侧符号用出厂表、中/英在右边的那一副。 */
        public Preview(Kind kind, TouchKeyStyle.Styles styles, boolean numberRow, boolean eighteen, boolean symbolFirst,
                boolean swapLanguageNumber) {
            this(kind, styles, numberRow, eighteen, symbolFirst, swapLanguageNumber, List.of());
        }

        /** 中/英在右边的那一副。 */
        public Preview(Kind kind, TouchKeyStyle.Styles styles, boolean numberRow, boolean eighteen, boolean symbolFirst,
                boolean swapLanguageNumber, List<String> nineKeySidebar) {
            this(kind, styles, numberRow, eighteen, symbolFirst, swapLanguageNumber, nineKeySidebar, false);
        }

        /** 只换画哪一类键盘，其余偏好不变；AI 皮肤页切「26 键 | 9 键」时用。 */
        public Preview with(Kind next) {
            return new Preview(next, styles, numberRow, eighteen, symbolFirst, swapLanguageNumber, nineKeySidebar,
                languageLeft);
        }

        /** 自上而下逐行的键，与 {@link KeyboardPreview} 画的相同。 */
        public List<List<Key>> rows() {
            List<List<Key>> rows = keyboard(kind, kind.selected(styles), numberRow, eighteen, symbolFirst,
                languageLeft, nineKeySidebar);
            return swapLanguageNumber ? swappedLanguageNumber(rows) : rows;
        }

        /** 社区皮肤详情在空格上标的那一句，说的是画的哪种键盘：九键、14 键、18 键或 26 键。 */
        public String caption() {
            return switch (kind) {
                case NINE -> "九键";
                case FOURTEEN -> eighteen ? "18 键" : "14 键";
                case TWENTY_SIX -> "26 键";
            };
        }
    }

    /** 预览画哪一类键盘：用户当前在用的中文键盘类型，判断的规则与「按键布局」页默认停的那一类相同（{@link #initialKind}）；判断不了或本版本一类都没有时画 26 键。 */
    public static Kind previewKind(TouchKeyStyle.Groups groups, KeyboardScheme current, String shuangpinLayout,
            String wubiLayout) {
        Kind kind = initialKind(offered(groups), current, shuangpinLayout, wubiLayout);
        return kind == null ? Kind.TWENTY_SIX : kind;
    }

    /** 由共享偏好里读出的几项定下预览（{@link Preview}）；读偏好的那一半在 {@link KeyboardPreview#layout} 里。 */
    public static Preview preview(TouchKeyStyle.Groups groups, KeyboardScheme current, String shuangpinLayout,
            String wubiLayout, TouchKeyStyle.Styles styles, boolean numberRow, boolean symbolFirst,
            boolean swapLanguageNumber, List<String> nineKeySidebar) {
        return preview(groups, current, shuangpinLayout, wubiLayout, styles, numberRow, symbolFirst, swapLanguageNumber,
            nineKeySidebar, false);
    }

    /** 同上，另带「中/英键放在空格左边」。 */
    public static Preview preview(TouchKeyStyle.Groups groups, KeyboardScheme current, String shuangpinLayout,
            String wubiLayout, TouchKeyStyle.Styles styles, boolean numberRow, boolean symbolFirst,
            boolean swapLanguageNumber, List<String> nineKeySidebar, boolean languageLeft) {
        return new Preview(previewKind(groups, current, shuangpinLayout, wubiLayout), styles, numberRow,
            eighteen(current, shuangpinLayout), symbolFirst, swapLanguageNumber, nineKeySidebar, languageLeft);
    }

    /** 每一行里同时有 123（`SoftLayer`）和中/英（`SoftLanguage`）时对调两键，宽度跟着键走；右侧大回车外框九宫格下方那一行同样对调，与键盘的 `tallReturnEntries` 一致。 */
    private static List<List<Key>> swappedLanguageNumber(List<List<Key>> rows) {
        List<List<Key>> result = new ArrayList<>(rows.size());
        for (List<Key> row : rows) {
            int layer = -1, language = -1;
            for (int index = 0; index < row.size(); index++) {
                if (LAYER.equals(row.get(index).id())) layer = index;
                if (LANGUAGE.equals(row.get(index).id())) language = index;
            }
            if (layer < 0 || language < 0) {
                result.add(row);
                continue;
            }
            List<Key> swapped = new ArrayList<>(row);
            java.util.Collections.swap(swapped, layer, language);
            result.add(List.copyOf(swapped));
        }
        return List.copyOf(result);
    }

    private static TouchKeyStyle.Row offered(Kind kind, TouchKeyStyle.Row style) {
        return kind.rows.contains(style) ? style : TouchKeyStyle.Row.STANDARD;
    }

    private static List<List<Key>> converted(List<List<KeyHeatmapLayout.Key>> layout) {
        List<List<Key>> rows = new ArrayList<>(layout.size());
        for (List<KeyHeatmapLayout.Key> source : layout) {
            List<Key> row = new ArrayList<>(source.size());
            for (KeyHeatmapLayout.Key key : source) row.add(converted(key));
            rows.add(List.copyOf(row));
        }
        return List.copyOf(rows);
    }

    private static Key converted(KeyHeatmapLayout.Key key) {
        String id = key.ids().isEmpty() ? "" : key.ids().get(0);
        if (SEPARATOR.equals(id)) return new Key("⇧", key.weight(), SHIFT, Tone.FUNCTION, key.rows());
        return new Key(key.face(), key.weight(), id, tone(id, key.face()), key.rows());
    }

    /** 键的底色按 id 定：回车是强调色，⇧ ⌫ 符 123 中/英 😀 和不计数的「重输」是功能键，其余（字母、数字、标点、空格、九宫格）是字符键。 */
    static Tone tone(String id, String face) {
        if (RETURN.equals(id)) return Tone.RETURN;
        if (FUNCTION_IDS.contains(id) || id.isEmpty() && !face.isEmpty()) return Tone.FUNCTION;
        return Tone.CHARACTER;
    }

    private static List<List<Key>> fourteen(TouchKeyStyle.Row style, boolean numberRow, boolean eighteen,
            boolean languageLeft) {
        List<List<FourteenKeyLayout.Key>> letters = eighteen ? EighteenKeyLayout.rows() : FourteenKeyLayout.rows();
        // 与 ImeLetterRows#rebuildFourteenKeyRows 相同：14 键每个字母键两份、两端 1.4 份；18 键字母区铺满整宽，第二排不再缩进。
        float letterWeight = eighteen ? 1f : FourteenKeyLayout.LETTER_WEIGHT;
        float edgeWeight = eighteen ? 1f : KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT;
        List<String> sidebar = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        List<List<Key>> rows = new ArrayList<>(letters.size() + 2);
        if (numberRow) rows.add(numberRow());
        for (int index = 0; index < letters.size(); index++) {
            boolean last = index == letters.size() - 1;
            List<Key> cells = new ArrayList<>(letters.get(index).size() + 2);
            if (last) cells.add(new Key(FourteenKeyLayout.separatorFace(false), edgeWeight, "SoftSymbol", Tone.FUNCTION, 1));
            for (FourteenKeyLayout.Key key : letters.get(index)) {
                cells.add(new Key(FourteenKeyLayout.face(key), letterWeight,
                    eighteen ? EighteenKeyLayout.keyId(key) : FourteenKeyLayout.keyId(key), Tone.CHARACTER, 1));
            }
            if (last) cells.add(new Key("⌫", edgeWeight, "Backspace", Tone.FUNCTION, 1));
            // 14 键左边接上符号栏这一格；18 键没有侧栏，三行都从同一左边界铺满。
            float total = 0;
            for (Key cell : cells) total += cell.weight();
            float scale = eighteen ? 1f / total : FourteenKeyLayout.LETTER_COLUMN_WEIGHT / total;
            List<Key> row = new ArrayList<>(cells.size() + 1);
            if (!eighteen) {
                row.add(new Key(index < sidebar.size() ? sidebar.get(index) : "", FourteenKeyLayout.SPELLING_SIDEBAR_WEIGHT,
                    index < sidebar.size() ? "SoftPunctuation" : "", Tone.CHARACTER, index < sidebar.size() ? 1 : 0));
            }
            for (Key cell : cells) row.add(new Key(cell.face(), cell.weight() * scale, cell.id(), cell.tone(), cell.rows()));
            rows.add(List.copyOf(row));
        }
        rows.add(converted(List.of(KeyHeatmapLayout.bottom(KeyboardLayout.FOURTEEN_KEY_LAYOUT, style, false,
            languageLeft))).get(0));
        return List.copyOf(rows);
    }

    private static List<Key> numberRow() {
        List<Key> row = new ArrayList<>(KeyboardLayout.NUMBER_ROW.size());
        for (String digit : KeyboardLayout.NUMBER_ROW)
            row.add(new Key(digit, 1f, KeyPressIds.forCharacter(digit.charAt(0)), Tone.CHARACTER, 1));
        return List.copyOf(row);
    }
}
