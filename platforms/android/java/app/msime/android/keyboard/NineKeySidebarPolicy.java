package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;

/**
 * 九键左侧符号栏的内容：字母键面与九宫格数字键盘各自的出厂符号，以及用户在「按键布局」里自定义的两张表的读取与校验。两张表是共享偏好 {@link #LETTER_PREFERENCE}、{@link #DIGIT_PREFERENCE}（JSON 字符串数组，空表表示用本端出厂表），随账号同步为 `platform.android.nine_key_symbols` / `platform.android.nine_key_digit_symbols`（符号用一个空格连起来的写法）；本机设置文件里同名的两个旧键只由 {@code KeyLayoutStyleMigration} 读一次、迁完即删（#10223）。
 *
 * <p>数字键面的符号栏一屏最多 {@link #DIGIT_VISIBLE_ROWS} 项，其他键面最多 {@link #VISIBLE_ROWS} 项，多出来的上下滚动。数字键面用四则运算符号，叹号挪到运算符号后面，滚动才看得到（#5590）。上限 {@link #MAX_SYMBOLS}、{@link #MAX_SYMBOL_CODE_POINTS}、{@link #MAX_TEXT_LENGTH} 与 client-core 的 `NINE_KEY_SIDEBAR_*` 常量相同，iOS、鸿蒙的编辑器用同样的数，`scripts/test-nine-key-sidebar-parity.py` 按这些名字核对。
 */
public final class NineKeySidebarPolicy {
    /** 字母键面那张表的共享偏好（client-core `touch_nine_key_sidebar_symbols`）。 */
    public static final String LETTER_PREFERENCE = "touch_nine_key_sidebar_symbols";
    /** 九宫格数字键盘那张表的共享偏好（client-core `touch_nine_key_digit_sidebar_symbols`）。 */
    public static final String DIGIT_PREFERENCE = "touch_nine_key_digit_sidebar_symbols";
    /** 长按符号栏打开「按键布局」页时带的页面参数，值是要直接编辑的那张表的共享偏好名（{@link #LETTER_PREFERENCE} 或 {@link #DIGIT_PREFERENCE}）。只用于导航：页面据此打开编辑页，确认前什么也不写。 */
    public static final String EDIT_ARGUMENT = "edit_sidebar_key";
    /** 符号栏一屏最多显示几个符号；符号不足这么多时按实际个数均分整栏高度。 */
    public static final int VISIBLE_ROWS = 5;
    /** 数字键面一屏最多五项，和九键左侧符号栏一致；最小行高规则仍可减少可见项。 */
    public static final int DIGIT_VISIBLE_ROWS = 5;
    /** 每个符号键至少多高（dp）：键盘高度调到最矮时一屏少放几个，不把键压得比字还矮。 */
    public static final int MIN_ROW_HEIGHT_DP = 28;
    /** 一张符号表最多几个符号。 */
    public static final int MAX_SYMBOLS = 30;
    /** 一个符号最多几个码位（「……」「:-)」这类短串也允许）。 */
    public static final int MAX_SYMBOL_CODE_POINTS = 4;
    /** 一张表用一个空格连起来的最大长度（UTF-16 单元），也是同步值的长度上限。 */
    public static final int MAX_TEXT_LENGTH = 200;

    /** 字母键面默认只留三个常用符号，与九键三行对齐；！ 已在右列，不在这里重复。 */
    public static final List<String> DEFAULT_LETTER_SYMBOLS =
        List.of("，", "。", "？");
    /** 右侧大回车样式（{@link TouchKeyStyle.Row#TALL_RETURN}）去掉了右列的 ！，它接在字母键面出厂符号的末尾（{@link #letterSymbols(List, boolean)}）；自定义的符号表原样使用。 */
    public static final List<String> TALL_RETURN_LETTER_SYMBOLS =
        List.of("，", "。", "？", "！");
    /** 数字键面的默认符号：先是四则运算和算式里常用的半角符号，中文标点（含原来在右列的 ！）排在后面。 */
    public static final List<String> DEFAULT_DIGIT_SYMBOLS =
        List.of("+", "-", "*", "/", "=", "%", "(", ")", ":", "@", "，", "。", "？", "！");
    /** 编辑页可直接加入的常用符号，三端逐项相同（iOS、共享设置页照抄这张表）。 */
    public static final List<String> ADDABLE_SYMBOLS = List.of(
        "，", "。", "？", "！", "、", "；", "：", "…", "……", "—", "·",
        "“", "”", "‘", "’", "（", "）", "（）", "《", "》", "【", "】", "～",
        "+", "-", "*", "/", "=", "%", "(", ")", "()", ":", "@", "#", "&",
        "_", "|", "<", ">", "^", "￥", "★", "☆", "♡", "♥", "→", "←", "↑", "↓", ":-)");

    private NineKeySidebarPolicy() {}

    /**
     * 把一段文本（本机旧键的值、手动输入的一项）解析成符号表：按空白（含全角空格）分开，去掉重复项，保留第一次出现的顺序。整段有一处不合规就整段不要，这一点和 client-core 逐项丢弃的规范化不同：编辑器要的是「能不能确认」。
     *
     * @return 符号表；文本为空、有符号超过 {@link #MAX_SYMBOL_CODE_POINTS} 个码位或含控制字符、符号超过 {@link #MAX_SYMBOLS} 个、文本超过 {@link #MAX_TEXT_LENGTH} 时返回 null
     */
    public static List<String> parse(String text) {
        if (text == null || text.length() > MAX_TEXT_LENGTH) return null;
        Set<String> symbols = new LinkedHashSet<>();
        StringBuilder token = new StringBuilder();
        int index = 0;
        while (index <= text.length()) {
            int codePoint = index < text.length() ? text.codePointAt(index) : ' ';
            if (TextPolicy.isSpace(codePoint)) {
                if (token.length() > 0) {
                    symbols.add(token.toString());
                    token.setLength(0);
                }
            } else {
                if (TextPolicy.isControl(codePoint)) return null;
                token.appendCodePoint(codePoint);
                if (TextPolicy.codePointLength(token.toString()) > MAX_SYMBOL_CODE_POINTS) return null;
            }
            index += index < text.length() ? Character.charCount(codePoint) : 1;
        }
        if (symbols.isEmpty() || symbols.size() > MAX_SYMBOLS) return null;
        return List.copyOf(symbols);
    }

    /** 本机旧键的规范文本：解析后的符号以一个空格连接；不合规时为 null。只剩 {@code AndroidLocalSettings} 读旧键时用它。 */
    public static String normalize(String text) {
        List<String> symbols = parse(text);
        return symbols == null ? null : format(symbols);
    }

    /** 符号表的文本写法：符号以一个空格连接，空表是空串；账号同步的值、旧键迁移和备份恢复都用这个写法。 */
    public static String format(List<String> symbols) {
        return String.join(" ", symbols);
    }

    /**
     * 共享偏好里存的一张表（JSON 字符串数组）读成符号表：不是数组时是空表，数组里不是字符串的元素丢掉，其余原样保留（client-core 读入时已经逐项规范化）。空表表示用本端出厂表。
     */
    public static List<String> fromPreference(Object value) {
        if (!(value instanceof JSONArray array)) return List.of();
        List<Object> items = new ArrayList<>(array.length());
        for (int index = 0; index < array.length(); index++) items.add(array.opt(index));
        return stored(items);
    }

    /** 同上，按已经取出来的数组元素读：只留字符串。 */
    public static List<String> stored(List<?> items) {
        List<String> symbols = new ArrayList<>(items.size());
        for (Object item : items) if (item instanceof String symbol) symbols.add(symbol);
        return List.copyOf(symbols);
    }

    /** 编辑器确认后写进共享偏好的表：与本端出厂表逐项、同顺序相同时写空表（「恢复默认」也是空表），否则写用户的表。 */
    public static List<String> toPreference(List<String> symbols, List<String> defaults) {
        return symbols.equals(defaults) ? List.of() : List.copyOf(symbols);
    }

    /** 存的表是不是用户自定义的：不是空表，也不与本端出厂表相同。别处写下与出厂表相同的非空表时仍按出厂表算（右侧大回车照样接 ！，预览不高亮）。 */
    public static boolean custom(List<String> stored, List<String> defaults) {
        return !stored.isEmpty() && !stored.equals(defaults);
    }

    /** 实际显示的符号：空表回到 `fallback`，其余原样使用。 */
    public static List<String> symbols(List<String> stored, List<String> fallback) {
        return stored.isEmpty() ? fallback : stored;
    }

    /** 字母键面（拼音九键、九键双拼、笔画、14 键左侧底轨）显示的符号。 */
    public static List<String> letterSymbols(List<String> stored) {
        return letterSymbols(stored, false);
    }

    /** 同上；{@code tallReturn} 为真（右侧大回车外框）时出厂表末尾多一个 ！，自定义的表原样使用、不接 ！。 */
    public static List<String> letterSymbols(List<String> stored, boolean tallReturn) {
        if (custom(stored, DEFAULT_LETTER_SYMBOLS)) return stored;
        return tallReturn ? TALL_RETURN_LETTER_SYMBOLS : DEFAULT_LETTER_SYMBOLS;
    }

    /** 右侧大回车外框接到出厂字母表末尾的那个 ！ 在表里的位置（{@link #letterSymbols(List, boolean)}）；数字键面、别的样式和自定义的表没有接，返回 -1。它接替拼音九键右列的 ！，带着那个键滑出 0（方向跟「九键滑动输入数字」，#10441）、长按选上标 ⁰ 或下标 ₀ 的手势；自定义表里的 ！ 是普通符号，不带。 */
    public static int appendedExclamationIndex(boolean digits, List<String> letterStored, boolean tallReturn) {
        return !digits && tallReturn && !custom(letterStored, DEFAULT_LETTER_SYMBOLS)
            ? TALL_RETURN_LETTER_SYMBOLS.size() - 1 : -1;
    }

    /** 数字键面（所有九宫格数字键盘的左列）显示的符号。 */
    public static List<String> digitSymbols(List<String> stored) {
        return symbols(stored, DEFAULT_DIGIT_SYMBOLS);
    }

    /**
     * 当前键面的左栏符号：九宫格数字键盘（拼音九键、九键双拼、笔画的数字层，以及其他键面按 123 借来的那一块）用数字那张表（#5590），字母键面用字母那张。
     *
     * @param digits 是不是九宫格数字键盘
     * @param letterStored 共享偏好 {@link #LETTER_PREFERENCE} 的表
     * @param digitStored 共享偏好 {@link #DIGIT_PREFERENCE} 的表
     */
    public static List<String> sidebarSymbols(boolean digits, List<String> letterStored, List<String> digitStored) {
        return sidebarSymbols(digits, letterStored, digitStored, false);
    }

    /** 同上；{@code tallReturn} 为真时字母键面用 {@link #letterSymbols(List, boolean)} 的那张表，数字键面的表本来就有 ！，不变。 */
    public static List<String> sidebarSymbols(boolean digits, List<String> letterStored, List<String> digitStored,
            boolean tallReturn) {
        return digits ? digitSymbols(digitStored) : letterSymbols(letterStored, tallReturn);
    }

    /** 编辑器里整张表能不能确认（与 {@link #parse} 同一套规则）：不是空表，不超过 {@link #MAX_SYMBOLS} 项，每项都是一个合规的符号（不超过 {@link #MAX_SYMBOL_CODE_POINTS} 个码位，不含空白和控制字符），没有重复，用一个空格连起来不超过 {@link #MAX_TEXT_LENGTH} 个 UTF-16 单元。 */
    public static boolean valid(List<String> symbols) {
        if (symbols.isEmpty()) return false;
        for (String symbol : symbols) {
            List<String> single = parse(symbol);
            if (single == null || single.size() != 1 || !single.get(0).equals(symbol)) return false;
        }
        List<String> parsed = parse(format(symbols));
        return parsed != null && parsed.equals(symbols);
    }

    /** 手动输入的一项（去掉首尾空白后）能不能接到当前表末尾：它本身是一个合规的符号、表里还没有它，接上后整张表仍然合规；能接时返回去掉空白的符号，否则为 null。 */
    public static String addable(List<String> current, String input) {
        if (input == null) return null;
        List<String> single = parse(input);
        if (single == null || single.size() != 1) return null;
        String symbol = single.get(0);
        if (current.contains(symbol)) return null;
        List<String> next = new ArrayList<>(current);
        next.add(symbol);
        return valid(next) ? symbol : null;
    }

    /**
     * 符号栏里每个符号占的高度（像素）：一屏放 {@link #VISIBLE_ROWS} 个，符号少于这么多时均分整栏，不留空白；放这么多会让每个低于 `minRowHeight` 时少放几个。
     *
     * @param railHeight 符号栏的可见高度
     * @param count 符号个数
     * @param minRowHeight 每个符号的最小高度（像素，{@link #MIN_ROW_HEIGHT_DP} 换算后）
     */
    public static int rowHeight(int railHeight, int count, int minRowHeight) {
        return rowHeight(railHeight, count, minRowHeight, VISIBLE_ROWS);
    }

    /** 按当前键面的可见行数分配符号高度，少量符号和最小高度仍按原规则处理。 */
    public static int rowHeight(int railHeight, int count, int minRowHeight, int visibleRows) {
        if (railHeight <= 0) return 0;
        int rows = BoundsPolicy.atMost(count, BoundsPolicy.atLeast(visibleRows, 1));
        if (minRowHeight > 0) rows = BoundsPolicy.atMost(rows, railHeight / minRowHeight);
        return railHeight / BoundsPolicy.atLeast(rows, 1);
    }

    /** 列出时给设置页显示的摘要：最多前 `limit` 个符号，后面用「等 N 个」收尾。 */
    public static String summary(List<String> symbols, int limit) {
        if (symbols.size() <= limit) return String.join(" ", symbols);
        List<String> head = new ArrayList<>(symbols.subList(0, BoundsPolicy.nonNegative(limit)));
        return String.join(" ", head) + " 等 " + symbols.size() + " 个";
    }

    /** 编辑页「当前顺序」每行符号前的序号：第 `position` 位（从 0 起）写成「1.」「2.」…，三端同一写法（iOS `NineKeySidebar.ordinal`、共享设置页 `nineKeySidebarOrdinal`）。 */
    public static String ordinal(int position) {
        return (position + 1) + ".";
    }

    /** 读屏念的位置：「第 3 项」。拖动把手把它当状态念，读屏用「上移」「下移」挪过之后焦点落在把手上，听得到挪到了哪一位。 */
    public static String positionDescription(int position) {
        return "第 " + (position + 1) + " 项";
    }

    /** 读屏念的一行：「第 3 项，符号：？」。序号本身对读屏隐藏，位置只在这里念一次。 */
    public static String ordinalDescription(int position, String symbol) {
        return positionDescription(position) + "，符号：" + symbol;
    }

    /**
     * 拖动时手指贴近滚动区上下边缘，每一帧让滚动区滚多少（像素，负数朝上）：进入边缘带越深滚得越快，到边缘外就是最快的 `maxStep`；不在边缘带里是 0。能不能再往那个方向滚由调用方判断。
     *
     * @param y 手指的屏幕纵坐标
     * @param top 滚动区可见部分的上沿
     * @param bottom 滚动区可见部分的下沿
     * @param edge 边缘带的高度
     * @param maxStep 每帧最多滚多少
     */
    public static int edgeScrollStep(float y, float top, float bottom, float edge, float maxStep) {
        if (edge <= 0 || bottom - top <= 2 * edge) return 0;
        float depth;
        if (y < top + edge) depth = -(top + edge - y);
        else if (y > bottom - edge) depth = y - (bottom - edge);
        else return 0;
        float ratio = Math.min(1f, Math.abs(depth) / edge);
        int step = Math.max(1, Math.round(maxStep * ratio));
        return depth < 0 ? -step : step;
    }

    /**
     * 被拖行还能朝 `step` 的方向挪时才照 {@link #edgeScrollStep} 自动滚动，否则是 0：它已经夹在列表下沿（`offset` 到了 `maxOffset`）时再往下滚，或夹在上沿时再往上滚，列表带着它滚走，行就离开手指了。共享设置页的 `scrollTowardRoom` 是同一条规则。
     *
     * @param step {@link #edgeScrollStep} 给出的一步
     * @param offset 被拖行此刻的跟手位移
     * @param minOffset 位移的下限（被拖行到了列表上沿）
     * @param maxOffset 位移的上限（被拖行到了列表下沿）
     */
    public static int scrollTowardRoom(int step, float offset, float minOffset, float maxOffset) {
        if (step > 0 && offset >= maxOffset) return 0;
        if (step < 0 && offset <= minOffset) return 0;
        return step;
    }

    /** 把 `from` 那一项挪到 `to`，其余次序不变（挪动，不是交换）；与共享设置页的 `moved`、iOS `NineKeySidebarDraft.move` 同一语义。下标越界时原样返回一份拷贝。 */
    public static List<String> moved(List<String> symbols, int from, int to) {
        List<String> next = new ArrayList<>(symbols);
        if (from < 0 || from >= next.size() || to < 0 || to >= next.size()) return next;
        next.add(to, next.remove(from));
        return next;
    }

    /**
     * 拖动途中第 `index` 行此刻显示在第几位：被拖的那一行（`from`）在 `to`，夹在两者之间的行朝 `from` 那一侧让一格，其余不动。行的视图在拖动中不换位置，序号和让位位移都按它算，松手后才把表按 {@link #moved} 挪好。
     */
    public static int dragSlot(int index, int from, int to) {
        if (index == from) return to;
        if (from < index && index <= to) return index - 1;
        if (to <= index && index < from) return index + 1;
        return index;
    }

    /**
     * 拖动中的行该落在第几位：数其余各行里布局中线（不含让位动画的位移）在被拖行中线之上的有几行，被拖行的中线碰到哪一行的中线就算到了那一行（被拖行夹在列表上下沿之间，拖到底时中线正好落在最后一行的中线上）。各行中线都按布局取，所以结果只随手指走，不随别的行的动画抖动；被拖行自己不参与比较。
     *
     * @param draggedCenter 被拖行此刻的中线（跟手后的位置）
     * @param from 被拖行的原位
     * @param centers 各行在布局里的中线，与表同序
     */
    public static int dragTarget(float draggedCenter, int from, float[] centers) {
        int target = 0;
        for (int index = 0; index < centers.length; index++) {
            if (index < from ? centers[index] < draggedCenter : index > from && centers[index] <= draggedCenter) target++;
        }
        return target;
    }

    /** 返回尚未加入当前符号栏的可选项，保持选择面板的固定顺序。 */
    public static List<String> addableSymbols(List<String> current) {
        ArrayList<String> available = new ArrayList<>();
        for (String symbol : ADDABLE_SYMBOLS) if (!current.contains(symbol)) available.add(symbol);
        return List.copyOf(available);
    }
}
