package app.msime.android;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 统计页按键热力图画的两套键盘：竖屏默认设置下的中文全拼 26 键和拼音九键，键序、宽度份额、键面和每个键记的 id 都取自触屏键盘自己用的常量（{@link KeyboardLayout}、{@link KeyboardActionRow}、{@link NineKeyLayout}、{@link NineKeySidebarPolicy}），键盘改了，热力图跟着变。
 *
 * <p>纯 Java，不碰 android.graphics，所以 check-host 能在宿主机 JVM 上核对它和键盘一致。
 */
public final class KeyHeatmapLayout {
    /**
     * 一个画出来的键：键面、在这一行里的宽度份额，以及按它时会记下的 id。
     *
     * <p>多数键只记一个 id。26 键第三行左端在中文全拼下是「分词」、记 `Quote`；英文模式下同一键位是 ⇧、记 `ShiftLeft`，热力图画的是中文键盘，⇧ 不并进这一格而列进「其他键」，与 iOS、鸿蒙一致。九键右列的「重输」不计数，没有 id，画成 0 次那一档。
     */
    public record Key(String face, float weight, List<String> ids) {
        public Key {
            ids = List.copyOf(ids);
        }
    }

    /** 26 键第三行左端在中文全拼下的键面，和 `MSIMEInputService` 给 ⇧ 换上的字一致。 */
    static final String SEPARATOR_FACE = "分词";
    /** 九键右列中间那格，`ImeLayoutRows#rebuildNineKeyRows` 建的「重输」；它不登记 id，按了不计数。 */
    static final String REWRITE_FACE = "重输";

    /** 中文全拼 26 键：三行字母（中文模式画大写，第二行九个键铺满整行），第三行两端是分词 / ⇧ 和 ⌫，底行与键盘的功能行相同。 */
    public static final List<List<Key>> SOFT = soft();

    /** 拼音九键：左栏默认的三个符号、3×3 网格、右列 ⌫ / 重输 / 最后一个标点，三列逐行对齐；底行与键盘的功能行相同。 */
    public static final List<List<Key>> NINE = nine();

    private KeyHeatmapLayout() {}

    private static List<List<Key>> soft() {
        List<List<String>> letters = KeyboardLayout.rows(KeyboardLayout.Layer.LETTERS);
        List<List<Key>> rows = new ArrayList<>(letters.size() + 1);
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
        rows.add(bottom(KeyboardLayout.STANDARD_TOUCH_LAYOUT));
        return List.copyOf(rows);
    }

    private static List<List<Key>> nine() {
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
        rows.add(bottom(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT));
        return List.copyOf(rows);
    }

    /** 默认设置下的功能行：不交换中/英和 123，Android 9 起不画地球键。 */
    private static List<Key> bottom(int touchLayout) {
        List<KeyboardActionRow.DesignEntry> entries = KeyboardActionRow.designEntries(touchLayout, false, false);
        List<Key> row = new ArrayList<>(entries.size());
        for (KeyboardActionRow.DesignEntry entry : entries) {
            row.add(switch (entry.slot()) {
                case SYMBOL_PANEL -> new Key("符", entry.weight(), List.of("SoftSymbol"));
                case LAYER -> new Key(KeyboardActionRow.layerTitle(touchLayout, false), entry.weight(),
                    List.of("SoftLayer"));
                case GLOBE -> new Key(KeyPressIds.label("SoftGlobe"), entry.weight(), List.of("SoftGlobe"));
                case COMMA -> new Key("，", entry.weight(), List.of("Comma"));
                case SPACE -> new Key("空格", entry.weight(), List.of("Space"));
                case ZERO -> new Key("0", entry.weight(), List.of("Nine0"));
                case PERIOD -> new Key("。", entry.weight(), List.of("Period"));
                case LANGUAGE -> new Key(LanguageKeyCyclePolicy.label(false, KeyboardScheme.QUANPIN, false,
                    List.of()), entry.weight(), List.of("SoftLanguage"));
                case RETURN -> new Key("↵", entry.weight(), List.of("Enter"));
            });
        }
        return List.copyOf(row);
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
        Set<String> drawn = new LinkedHashSet<>(ids(nine ? NINE : SOFT));
        List<String> result = new ArrayList<>(counts.size());
        for (Map.Entry<String, Long> entry : counts.entrySet()) {
            if (entry.getValue() > 0 && !drawn.contains(entry.getKey())) result.add(entry.getKey());
        }
        result.sort(Comparator.comparingLong((String id) -> counts.get(id)).reversed()
            .thenComparingInt(KeyPressIds.KEY_IDS::indexOf));
        return List.copyOf(result);
    }
}
