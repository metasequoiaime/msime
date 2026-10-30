package app.msime.android;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 统计页按键热力图画的两套键盘：竖屏默认设置下的中文全拼 26 键和拼音九键，键序、宽度份额、键面和每个键记的 id 都取自触屏键盘自己用的常量（{@link KeyboardLayout}、{@link KeyboardActionRow}、{@link NineKeyLayout}、{@link NineKeySidebarPolicy}），键盘改了，热力图跟着变。底行按用户当前选的底行样式画（{@link #soft(TouchKeyStyle.Row, boolean)}、{@link #nine(TouchKeyStyle.Row)}），26 键打开顶部数字行时在字母上方多画 1–0。
 *
 * <p>纯 Java，不碰 android.graphics，所以 check-host 能在宿主机 JVM 上核对它和键盘一致。
 */
public final class KeyHeatmapLayout {
    /**
     * 一个画出来的键：键面、在这一行里的宽度份额，以及按它时会记下的 id。
     *
     * <p>多数键只记一个 id。26 键第三行左端在中文全拼下是「分词」、记 `Quote`；英文模式下同一键位是 ⇧、记 `ShiftLeft`，热力图画的是中文键盘，⇧ 不并进这一格而列进「其他键」，与 iOS、鸿蒙一致。九键右列的「重输」不计数，没有 id，画成 0 次那一档。
     */
    public record Key(String face, float weight, List<String> ids, int rows) {
        public Key {
            ids = List.copyOf(ids);
            if (rows < 0 || rows > 2 || rows == 0 && !ids.isEmpty())
                throw new IllegalArgumentException("Invalid heatmap key span: " + rows);
        }

        /** 只占一行的键。 */
        public Key(String face, float weight, List<String> ids) {
            this(face, weight, ids, 1);
        }

        /** 占位：这一格被上一行跨下来的键（`rows == 2`）盖住，不画、不记 id，只占同样的宽度让同一行后面的键对齐。 */
        static Key covered(float weight) {
            return new Key("", weight, List.of(), 0);
        }
    }

    /** 26 键第三行左端在中文全拼下的键面，和 `MSIMEInputService` 给 ⇧ 换上的字一致。 */
    static final String SEPARATOR_FACE = "分词";
    /** 九键右列中间那格，`ImeLayoutRows#rebuildNineKeyRows` 建的「重输」；它不登记 id，按了不计数。 */
    static final String REWRITE_FACE = "重输";

    /** 中文全拼 26 键：三行字母（中文模式画大写，第二行九个键铺满整行），第三行两端是分词 / ⇧ 和 ⌫，底行与键盘的标准功能行相同。 */
    public static final List<List<Key>> SOFT = soft(TouchKeyStyle.Row.STANDARD, false);

    /** 拼音九键：左栏默认的三个符号、3×3 网格、右列 ⌫ / 重输 / 最后一个标点，三列逐行对齐；底行与键盘的标准功能行相同。 */
    public static final List<List<Key>> NINE = nine(TouchKeyStyle.Row.STANDARD);

    private KeyHeatmapLayout() {}

    /**
     * 中文全拼 26 键，底行按 `style` 排（与键盘的 {@link KeyboardActionRow#designEntries} 相同）；`numberRow` 为真时字母上方多一行 1–0，每个数字记 `Digit0`…`Digit9`，与键盘顶部数字行记的 id 相同。
     */
    public static List<List<Key>> soft(TouchKeyStyle.Row style, boolean numberRow) {
        return soft(style, numberRow, false);
    }

    /** 同上，`symbolFirst`（共享偏好 `touch_twenty_six_symbol_first`）为真时底行的 符 排在 123 左边，与键盘相同。 */
    public static List<List<Key>> soft(TouchKeyStyle.Row style, boolean numberRow, boolean symbolFirst) {
        return soft(style, numberRow, symbolFirst, false);
    }

    /** 同上，`languageLeft`（共享偏好 `touch_language_key_side` 为 `left`）为真时底行的中/英排到空格左边（{@link KeyboardActionRow#languageBeforeSpace}），与键盘相同。 */
    public static List<List<Key>> soft(TouchKeyStyle.Row style, boolean numberRow, boolean symbolFirst,
            boolean languageLeft) {
        List<List<String>> letters = KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS);
        List<List<Key>> rows = new ArrayList<>(letters.size() + 2);
        if (numberRow) {
            List<Key> digits = new ArrayList<>(KeyboardLayout.NUMBER_ROW.size());
            for (String digit : KeyboardLayout.NUMBER_ROW)
                digits.add(new Key(digit, 1f, List.of(KeyPressIds.forCharacter(digit.charAt(0)))));
            rows.add(List.copyOf(digits));
        }
        for (int index = 0; index < letters.size(); index++) {
            List<Key> row = new ArrayList<>(letters.get(index).size() + 2);
            boolean last = index == letters.size() - 1;
            if (last) row.add(new Key(SEPARATOR_FACE, KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT,
                List.of("Quote")));
            for (String letter : letters.get(index)) {
                row.add(new Key(LetterKeyFacePolicy.face(letter, true, false, false, false), 1f,
                    List.of(KeyPressIds.forCharacter(letter.charAt(0)))));
            }
            if (last) row.add(new Key("⌫", KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT, List.of("Backspace")));
            rows.add(List.copyOf(row));
        }
        rows.add(bottom(KeyboardLayout.STANDARD_TOUCH_LAYOUT, style, symbolFirst, languageLeft));
        return List.copyOf(rows);
    }

    /** 拼音九键，底行按 `style` 排；除右侧大回车（{@link #tallReturnNine}）外，左栏、网格和右列不随样式变。 */
    public static List<List<Key>> nine(TouchKeyStyle.Row style) {
        return nine(style, false);
    }

    /** 同上，`languageLeft`（共享偏好 `touch_language_key_side` 为 `left`）为真时底行（右侧大回车是九宫格下方那一行）的中/英排到空格左边，与键盘相同。 */
    public static List<List<Key>> nine(TouchKeyStyle.Row style, boolean languageLeft) {
        if (KeyboardActionRow.drawsTallReturn(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, style)) return tallReturnNine(languageLeft);
        List<List<NineKeyLayout.Key>> grid = NineKeyLayout.rows();
        List<String> sidebar = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        List<String> punctuation = NineKeyLayout.punctuation();
        // 右列自上而下：⌫、重输、标点表的最后一个（！）。
        List<Key> actions = List.of(
            new Key("⌫", NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of("Backspace")),
            new Key(REWRITE_FACE, NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of()),
            new Key(punctuation.get(punctuation.size() - 1), NineKeyLayout.SIDE_COLUMN_WEIGHT,
                List.of("SoftPunctuation")));
        float cellWeight = NineKeyLayout.GRID_WEIGHT / grid.get(0).size();
        List<List<Key>> rows = new ArrayList<>(grid.size() + 1);
        for (int index = 0; index < grid.size(); index++) {
            List<Key> row = new ArrayList<>(grid.get(index).size() + 2);
            // 左栏的符号少于一屏时均分整栏，默认三个正好和网格三行对齐。
            row.add(new Key(sidebar.get(index), NineKeyLayout.SIDE_COLUMN_WEIGHT, List.of("SoftPunctuation")));
            for (NineKeyLayout.Key key : grid.get(index)) {
                row.add(new Key(NineKeyLayout.face(key, false), cellWeight,
                    List.of(KeyPressIds.forNineKeyDigit(key.digit()))));
            }
            row.add(actions.get(index));
            rows.add(List.copyOf(row));
        }
        rows.add(bottom(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, style, false, languageLeft));
        return List.copyOf(rows);
    }

    /**
     * 右侧大回车的拼音九键外框，和键盘的 `ImeLayoutRows` 排的一样：右列自上而下 ⌫、重输，回车（`Enter`）从第三行跨到底行（{@link Key#rows} 为 2，底行那一格是 {@link Key#covered} 占位）；底行最左是与左栏同宽的 符（`SoftSymbol`），九宫格正下方是 {@link KeyboardActionRow#tallReturnEntries}，份额换算到与九宫格同宽。
     *
     * <p>左栏的默认符号是 ，。？！ 四个，键盘上四个均分整栏高度，这里逐行只画得下前三个；！ 与它们同记 `SoftPunctuation`，按它的次数照样落在这三格的颜色里（{@link #sharedIds}）。
     */
    private static List<List<Key>> tallReturnNine(boolean languageLeft) {
        List<List<NineKeyLayout.Key>> grid = NineKeyLayout.rows();
        List<String> sidebar = NineKeySidebarPolicy.TALL_RETURN_LETTER_SYMBOLS;
        float side = NineKeyLayout.SIDE_COLUMN_WEIGHT;
        List<Key> actions = List.of(
            new Key("⌫", side, List.of("Backspace")),
            new Key(REWRITE_FACE, side, List.of()),
            new Key("↵", side, List.of("Enter"), 2));
        float cellWeight = NineKeyLayout.GRID_WEIGHT / grid.get(0).size();
        List<List<Key>> rows = new ArrayList<>(grid.size() + 1);
        for (int index = 0; index < grid.size(); index++) {
            List<Key> row = new ArrayList<>(grid.get(index).size() + 2);
            row.add(new Key(sidebar.get(index), side, List.of("SoftPunctuation")));
            for (NineKeyLayout.Key key : grid.get(index)) {
                row.add(new Key(NineKeyLayout.face(key, false), cellWeight,
                    List.of(KeyPressIds.forNineKeyDigit(key.digit()))));
            }
            row.add(actions.get(index));
            rows.add(List.copyOf(row));
        }
        List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.tallReturnEntries(
            NineKeyLayout.GRID_WEIGHT / (2 * side + NineKeyLayout.GRID_WEIGHT), false, false, false, languageLeft);
        float total = 0;
        for (KeyboardActionRow.DesignEntry entry : entries) total += entry.weight();
        List<Key> bottom = new ArrayList<>(entries.size() + 2);
        bottom.add(new Key("符", side, List.of("SoftSymbol")));
        for (KeyboardActionRow.DesignEntry entry : entries) {
            bottom.add(key(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, new KeyboardActionRow.DesignEntry(entry.slot(),
                entry.weight() * NineKeyLayout.GRID_WEIGHT / total)));
        }
        bottom.add(Key.covered(side));
        rows.add(List.copyOf(bottom));
        return List.copyOf(rows);
    }

    /** 默认设置下的功能行：不交换中/英和 123，Android 9 起不画地球键；键的排法跟着底行样式。「按键布局」页画 14 键与 18 键的预览时也用这一行（{@code KeyLayoutStylesPolicy}）。 */
    public static List<Key> bottom(int touchLayout, TouchKeyStyle.Row style) {
        return bottom(touchLayout, style, false, false);
    }

    /** 同上，另按「符」与「123」互换（`symbolFirst`）和中/英在空格左边（`languageLeft`）排，与键盘的 {@link KeyboardActionRow#designEntries} 相同。 */
    public static List<Key> bottom(int touchLayout, TouchKeyStyle.Row style, boolean symbolFirst, boolean languageLeft) {
        List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.designEntries(touchLayout, false, false,
            false, false, style, symbolFirst, TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, languageLeft);
        List<Key> row = new ArrayList<>(entries.size());
        for (KeyboardActionRow.DesignEntry entry : entries) row.add(key(touchLayout, entry));
        return List.copyOf(row);
    }

    /** 底行一个键的键面与 id。 */
    private static Key key(int touchLayout, KeyboardActionRow.DesignEntry entry) {
        return switch (entry.slot()) {
            case SYMBOL_PANEL -> new Key("符", entry.weight(), List.of("SoftSymbol"));
            case LAYER -> new Key(KeyboardActionRow.layerTitle(touchLayout, false), entry.weight(),
                List.of("SoftLayer"));
            case GLOBE -> new Key(KeyPressIds.label("SoftGlobe"), entry.weight(), List.of("SoftGlobe"));
            // 表情键样式的 😀 与工具栏的表情按钮记同一个 id，存储分不出按的是哪一个，次数都落在这一格。
            case EMOJI -> new Key("😀", entry.weight(), List.of("SoftEmoji"));
            case COMMA -> new Key("，", entry.weight(), List.of("Comma"));
            case SPACE -> new Key("空格", entry.weight(), List.of("Space"));
            case ZERO -> new Key("0", entry.weight(), List.of("Nine0"));
            case PERIOD -> new Key("。", entry.weight(), List.of("Period"));
            case LANGUAGE -> new Key(LanguageKeyCyclePolicy.label(false, KeyboardScheme.QUANPIN, false,
                List.of()), entry.weight(), List.of("SoftLanguage"));
            // 只有手写底行带这两个键；热力图画的 26 键和九键底行不会走到这里。
            case REWRITE -> new Key("重写", entry.weight(), List.of());
            case DELETE -> new Key("⌫", entry.weight(), List.of("Backspace"));
            case RETURN -> new Key("↵", entry.weight(), List.of("Enter"));
        };
    }

    /** 一套布局里画出来的全部 id，按出现顺序、不重复。 */
    public static List<String> ids(List<List<Key>> layout) {
        List<String> result = new ArrayList<>();
        for (List<Key> row : layout) {
            for (Key key : row) {
                for (String id : key.ids()) if (!result.contains(id)) result.add(id);
            }
        }
        return List.copyOf(result);
    }

    /**
     * 一套布局里不止一个键记的 id，比如九键左栏的「，。？」和右列的「！」都记 `SoftPunctuation`。
     *
     * <p>存储分不出按的是其中哪一个，热力图给这些键涂同一档颜色，但哪个键上都不印百分数：只印在一个键上会被读成那个键自己的占比。和 HarmonyOS、iOS 的做法一致。
     */
    public static Set<String> sharedIds(List<List<Key>> layout) {
        Set<String> seen = new HashSet<>();
        Set<String> shared = new LinkedHashSet<>();
        for (List<Key> row : layout) {
            for (Key key : row) {
                for (String id : key.ids()) if (!seen.add(id)) shared.add(id);
            }
        }
        return Set.copyOf(shared);
    }

    /**
     * 这套布局的热力图上没有位置的已按键，按次数从多到少，给「其他键」列表用。
     *
     * @param nine 画的是拼音九键（{@link #NINE}）还是 26 键（{@link #SOFT}）
     */
    public static List<String> others(Map<String, Long> counts, boolean nine) {
        return others(counts, nine ? NINE : SOFT);
    }

    /** 同上，按实际画出来的那套布局（带着用户的底行样式和数字行）判断。 */
    public static List<String> others(Map<String, Long> counts, List<List<Key>> layout) {
        Set<String> drawn = new LinkedHashSet<>(ids(layout));
        List<String> result = new ArrayList<>(counts.size());
        for (Map.Entry<String, Long> entry : counts.entrySet()) {
            if (entry.getValue() > 0 && !drawn.contains(entry.getKey())) result.add(entry.getKey());
        }
        result.sort(Comparator.comparingLong((String id) -> counts.get(id)).reversed()
            .thenComparingInt(KeyPressIds.KEY_IDS::indexOf));
        return List.copyOf(result);
    }
}
