import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.KeyboardScheme;
import app.msime.android.TouchKeyStyle;
import app.msime.android.TouchKeyStyle.Row;
import app.msime.android.home.KeyLayoutStylesPolicy;
import app.msime.android.home.KeyLayoutStylesPolicy.Key;
import app.msime.android.home.KeyLayoutStylesPolicy.Kind;
import app.msime.android.home.KeyLayoutStylesPolicy.Tone;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

/**
 * 「按键布局」子页的规则（与 iOS、共享设置页同一份规格）：分段列哪几类、默认停在哪一类，每类的胶囊名字和顺序、选样式写哪一项偏好，高亮集合，说明的显示与隐藏，数字行只画在 26 和 14/18 键上，以及整块预览的键序、份额和跨行取自键盘的表。读写偏好和画图的那一半在 `KeyLayoutStylesPage`、`KeyLayoutPreview` 里，要设备，这里跑不了。
 */
public final class KeyLayoutStylesPolicySmoke {
    public static void main(String[] arguments) {
        kinds();
        initialKind();
        choices();
        notes();
        highlights();
        numberRow();
        twentySixKeyboard();
        fourteenKeyboard();
        nineKeyboard();
        System.out.println("key layout styles page: passed");
    }

    private static void kinds() {
        check(KeyLayoutStylesPolicy.offered(new TouchKeyStyle.Groups(true, true, true))
            .equals(List.of(Kind.TWENTY_SIX, Kind.FOURTEEN, Kind.NINE)), "all three kinds in order");
        check(KeyLayoutStylesPolicy.offered(new TouchKeyStyle.Groups(true, true, false))
            .equals(List.of(Kind.TWENTY_SIX, Kind.FOURTEEN)), "wubi has no nine-key segment");
        check(KeyLayoutStylesPolicy.offered(new TouchKeyStyle.Groups(false, false, true)).equals(List.of(Kind.NINE)),
            "a stroke-only edition has one kind, so no segmented control");
        check(KeyLayoutStylesPolicy.offered(new TouchKeyStyle.Groups(false, false, false)).isEmpty(), "no kinds");
        check(Kind.TWENTY_SIX.title.equals("26 键") && Kind.FOURTEEN.title.equals("14/18 键") && Kind.NINE.title.equals("9 键"),
            "segment titles");
    }

    private static void initialKind() {
        List<Kind> all = List.of(Kind.TWENTY_SIX, Kind.FOURTEEN, Kind.NINE);
        String six = "twenty_six_key";
        for (KeyboardScheme scheme : new KeyboardScheme[] {KeyboardScheme.QUANPIN, KeyboardScheme.XIAOHE,
                KeyboardScheme.MICROSOFT, KeyboardScheme.WUBI, KeyboardScheme.CANTONESE})
            check(KeyLayoutStylesPolicy.initialKind(all, scheme, six, six) == Kind.TWENTY_SIX, scheme + " opens on 26 键");
        for (KeyboardScheme scheme : new KeyboardScheme[] {KeyboardScheme.QUANPIN_FOURTEEN_KEY,
                KeyboardScheme.QUANPIN_EIGHTEEN_KEY})
            check(KeyLayoutStylesPolicy.initialKind(all, scheme, six, six) == Kind.FOURTEEN, scheme + " opens on 14/18 键");
        check(KeyLayoutStylesPolicy.initialKind(all, KeyboardScheme.XIAOHE, "fourteen_key", six) == Kind.FOURTEEN,
            "shuangpin on 14 keys opens on 14/18 键");
        check(KeyLayoutStylesPolicy.initialKind(all, KeyboardScheme.ZIRANMA, "eighteen_key", six) == Kind.FOURTEEN,
            "shuangpin on 18 keys opens on 14/18 键");
        check(KeyLayoutStylesPolicy.initialKind(all, KeyboardScheme.WUBI, six, "fourteen_key") == Kind.FOURTEEN,
            "wubi on 14 keys opens on 14/18 键");
        check(KeyLayoutStylesPolicy.initialKind(all, KeyboardScheme.QUANPIN, "fourteen_key", "fourteen_key") == Kind.TWENTY_SIX,
            "quanpin ignores the shuangpin and wubi layouts");
        for (KeyboardScheme scheme : new KeyboardScheme[] {KeyboardScheme.QUANPIN_NINE_KEY,
                KeyboardScheme.NINE_KEY_SHUANGPIN, KeyboardScheme.STROKE})
            check(KeyLayoutStylesPolicy.initialKind(all, scheme, six, six) == Kind.NINE, scheme + " opens on 9 键");
        for (KeyboardScheme scheme : new KeyboardScheme[] {KeyboardScheme.HANDWRITING, KeyboardScheme.JAPANESE,
                KeyboardScheme.KOREAN, KeyboardScheme.ZHUYIN})
            check(KeyLayoutStylesPolicy.initialKind(all, scheme, six, six) == Kind.TWENTY_SIX, scheme + " falls back to the first");
        check(KeyLayoutStylesPolicy.initialKind(List.of(Kind.TWENTY_SIX, Kind.FOURTEEN), KeyboardScheme.STROKE, six, six)
            == Kind.TWENTY_SIX, "a kind the edition does not offer falls back to the first");
        check(KeyLayoutStylesPolicy.initialKind(List.of(Kind.NINE), KeyboardScheme.QUANPIN, six, six) == Kind.NINE,
            "a single offered kind is the default");
        check(KeyLayoutStylesPolicy.initialKind(all, null, six, six) == Kind.TWENTY_SIX, "an unknown scheme falls back");
        check(KeyLayoutStylesPolicy.initialKind(List.of(), KeyboardScheme.QUANPIN, six, six) == null, "no kinds, no default");

        check(KeyLayoutStylesPolicy.eighteen(KeyboardScheme.QUANPIN_EIGHTEEN_KEY, six), "quanpin 18 draws the 18-key grid");
        check(KeyLayoutStylesPolicy.eighteen(KeyboardScheme.XIAOHE, "eighteen_key"), "shuangpin 18 draws the 18-key grid");
        check(!KeyLayoutStylesPolicy.eighteen(KeyboardScheme.XIAOHE, "fourteen_key")
            && !KeyLayoutStylesPolicy.eighteen(KeyboardScheme.QUANPIN_FOURTEEN_KEY, six)
            && !KeyLayoutStylesPolicy.eighteen(KeyboardScheme.QUANPIN, "eighteen_key")
            && !KeyLayoutStylesPolicy.eighteen(null, six), "everything else draws the 14-key grid");
    }

    private static void choices() {
        check(labels(Kind.TWENTY_SIX).equals(List.of("标准", "逗号句号在空格两侧", "长空格", "表情键")), "26-key chips");
        check(labels(Kind.FOURTEEN).equals(List.of("标准", "长空格", "表情键")), "14/18-key chips");
        check(labels(Kind.NINE).equals(List.of("标准", "底栏带标点", "表情键", "右侧大回车")), "nine-key chips");
        // 选样式写这一类自己的共享偏好，写的是 wire 值，另外两类不动。
        check(Kind.TWENTY_SIX.preference.equals("touch_twenty_six_key_style")
            && Kind.FOURTEEN.preference.equals("touch_fourteen_key_style")
            && Kind.NINE.preference.equals("touch_nine_key_style"), "each kind writes its own preference");
        TouchKeyStyle.Styles styles = new TouchKeyStyle.Styles(Row.LONG_SPACE, Row.EMOJI, Row.TALL_RETURN);
        check(Kind.TWENTY_SIX.selected(styles) == Row.LONG_SPACE && Kind.FOURTEEN.selected(styles) == Row.EMOJI
            && Kind.NINE.selected(styles) == Row.TALL_RETURN, "each kind reads its own style");
        check(Kind.TWENTY_SIX.with(styles, Row.EMOJI).equals(new TouchKeyStyle.Styles(Row.EMOJI, Row.EMOJI, Row.TALL_RETURN)),
            "choosing a 26-key style leaves the others");
        check(Kind.FOURTEEN.with(styles, Row.STANDARD).equals(new TouchKeyStyle.Styles(Row.LONG_SPACE, Row.STANDARD, Row.TALL_RETURN)),
            "choosing a 14/18-key style leaves the others");
        check(Kind.NINE.with(styles, Row.COMMA_PERIOD).equals(new TouchKeyStyle.Styles(Row.LONG_SPACE, Row.EMOJI, Row.COMMA_PERIOD)),
            "choosing a nine-key style leaves the others");
        check(Row.TALL_RETURN.wire.equals("tall_return") && Row.COMMA_PERIOD.wire.equals("comma_period"), "wire values");
    }

    private static void notes() {
        check("符号键移到工具栏。".equals(KeyLayoutStylesPolicy.note(Kind.TWENTY_SIX, Row.LONG_SPACE)), "26 long space note");
        check("长按 123 打开符号。".equals(KeyLayoutStylesPolicy.note(Kind.TWENTY_SIX, Row.EMOJI)), "26 emoji note");
        check("长按 123 打开符号。".equals(KeyLayoutStylesPolicy.note(Kind.FOURTEEN, Row.EMOJI)), "14/18 emoji note");
        check("回车竖占两行，符号键在左下角。".equals(KeyLayoutStylesPolicy.note(Kind.NINE, Row.TALL_RETURN)), "nine tall return note");
        int shown = 0;
        for (Kind kind : Kind.values()) {
            for (Row row : kind.rows) if (KeyLayoutStylesPolicy.note(kind, row) != null) shown++;
        }
        check(shown == 4, "only those four styles show a note");
        check(KeyLayoutStylesPolicy.note(Kind.FOURTEEN, Row.LONG_SPACE) == null, "14/18 long space keeps 符 on the keys, no note");
        check(KeyLayoutStylesPolicy.note(Kind.NINE, Row.EMOJI) == null, "nine-key emoji keeps 符 in the frame, no note");
        for (Kind kind : Kind.values())
            check(KeyLayoutStylesPolicy.note(kind, Row.STANDARD) == null, "standard has no note on " + kind);
    }

    private static void highlights() {
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.COMMA_PERIOD).equals(Set.of("Comma", "Period")), "26 ， 。");
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.LONG_SPACE).equals(Set.of("Space")), "26 space");
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.EMOJI).equals(Set.of("SoftEmoji")), "26 emoji");
        check(KeyLayoutStylesPolicy.highlights(Kind.FOURTEEN, Row.LONG_SPACE).equals(Set.of("Space")), "14/18 space");
        check(KeyLayoutStylesPolicy.highlights(Kind.FOURTEEN, Row.EMOJI).equals(Set.of("SoftEmoji")), "14/18 emoji");
        check(KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.COMMA_PERIOD).equals(Set.of("Comma", "Period")), "nine ， 。");
        check(KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.EMOJI).equals(Set.of("SoftEmoji")), "nine emoji");
        check(KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.TALL_RETURN).equals(Set.of("Enter")), "nine return");
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.TALL_RETURN).isEmpty()
            && KeyLayoutStylesPolicy.highlights(Kind.FOURTEEN, Row.COMMA_PERIOD).isEmpty(), "a style the kind lacks has none");
        for (Kind kind : Kind.values()) {
            check(KeyLayoutStylesPolicy.highlights(kind, Row.STANDARD).isEmpty(), "standard highlights nothing on " + kind);
            // 每个高亮的 id 在这个样式的预览里都有一个画出来的键，高亮不会落空。
            for (Row row : kind.rows) {
                List<String> drawn = new ArrayList<>();
                for (List<Key> keys : KeyLayoutStylesPolicy.keyboard(kind, row, false, false)) {
                    for (Key key : keys) if (key.rows() > 0) drawn.add(key.id());
                }
                for (String id : KeyLayoutStylesPolicy.highlights(kind, row))
                    check(drawn.contains(id), kind + " " + row + " draws its highlighted " + id);
            }
        }
    }

    private static void numberRow() {
        check(KeyLayoutStylesPolicy.drawsNumberRow(Kind.TWENTY_SIX, true) && KeyLayoutStylesPolicy.drawsNumberRow(Kind.FOURTEEN, true),
            "26 and 14/18 draw the number row when it is on");
        check(!KeyLayoutStylesPolicy.drawsNumberRow(Kind.NINE, true), "nine keys never draw the number row");
        for (Kind kind : Kind.values()) check(!KeyLayoutStylesPolicy.drawsNumberRow(kind, false), "off draws nothing on " + kind);
        List<String> digits = List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, true, false).get(0)).equals(digits),
            "26 keys put 1–0 on top");
        check(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, true, false).size() == 5
            && KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false).size() == 4, "26 keys grow by one row");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, Row.STANDARD, true, true).get(0)).equals(digits)
            && KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, Row.STANDARD, true, false).size() == 5, "14/18 keys put 1–0 on top");
        List<List<Key>> nine = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, true, false);
        check(nine.size() == 4 && !faces(nine.get(0)).equals(digits), "nine keys skip the number row even when it is on");
    }

    private static void twentySixKeyboard() {
        List<List<Key>> rows = KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false);
        check(faces(rows.get(0)).equals(List.of("Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P")), "first letter row");
        check(faces(rows.get(1)).equals(List.of("A", "S", "D", "F", "G", "H", "J", "K", "L")), "second letter row fills the row");
        check(faces(rows.get(2)).equals(List.of("⇧", "Z", "X", "C", "V", "B", "N", "M", "⌫")), "third row is ⇧ z–m ⌫");
        check(rows.get(2).get(0).weight() == KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT
            && rows.get(2).get(0).tone() == Tone.FUNCTION && rows.get(2).get(0).id().equals("ShiftLeft"), "⇧ is a wide function key");
        for (Row style : Kind.TWENTY_SIX.rows) {
            List<Key> bottom = KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, style, false, false).get(3);
            sameEntries(bottom, KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false, false, false, false, style),
                "26-key bottom row " + style);
        }
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false).get(3))
            .equals(List.of("123", "符", "空格", "，", "中", "↵")), "standard bottom row faces");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.LONG_SPACE, false, false).get(3))
            .equals(List.of("123", "，", "空格", "中", "↵")), "long space still reads 空格");
        Key enter = rows.get(3).get(rows.get(3).size() - 1);
        check(enter.tone() == Tone.RETURN && enter.id().equals("Enter"), "return is the accent key");
        check(rows.get(0).get(0).tone() == Tone.CHARACTER && rows.get(3).get(0).tone() == Tone.FUNCTION, "letters are keycaps, 123 is a function key");
    }

    private static void fourteenKeyboard() {
        List<List<Key>> fourteen = KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, Row.STANDARD, false, false);
        check(fourteen.size() == 4, "three letter rows and the bottom row");
        check(faces(fourteen.get(0)).equals(List.of("，", "QW", "ER", "TY", "UI", "OP")), "14-key first row with the sidebar");
        check(faces(fourteen.get(1)).equals(List.of("。", "AS", "DF", "GH", "JK", "L")), "14-key second row");
        check(faces(fourteen.get(2)).equals(List.of("？", "符", "ZX", "CV", "BN", "M", "⌫")), "14-key third row is 符 … ⌫");
        for (int index = 0; index < 3; index++) {
            List<Key> row = fourteen.get(index);
            check(near(row.get(0).weight(), .7f), "sidebar keeps the keyboard's share on row " + index);
            check(near(sum(row) - row.get(0).weight(), 3.8f), "letters fill the keyboard's letter column on row " + index);
        }
        // 14 键第三行：符 和 ⌫ 1.4 份，字母键两份，换算到字母列里比例不变。
        List<Key> third = fourteen.get(2);
        check(near(third.get(1).weight() / third.get(2).weight(), 1.4f / 2f), "14-key edge keys keep 1.4 : 2");
        for (Row style : Kind.FOURTEEN.rows) {
            List<List<Key>> rows = KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, style, false, false);
            sameEntries(rows.get(3), KeyboardActionRow.designEntries(KeyboardLayout.FOURTEEN_KEY_LAYOUT, false, false, false, false, style),
                "14/18-key bottom row " + style);
        }
        List<List<Key>> eighteen = KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, Row.STANDARD, false, true);
        check(faces(eighteen.get(0)).equals(List.of("，", "Q", "WE", "RT", "Y", "U", "IO", "P")), "18-key first row");
        List<Key> middle = eighteen.get(1);
        check(middle.get(1).rows() == 0 && middle.get(middle.size() - 1).rows() == 0
            && near(middle.get(1).weight() / middle.get(2).weight(), .5f), "18-key second row is indented half a key each side");
        check(faces(eighteen.get(2)).equals(List.of("？", "符", "Z", "XC", "V", "BN", "M", "⌫")), "18-key third row");
        check(eighteen.get(1).get(2).id().equals("EighteenA") && fourteen.get(1).get(1).id().equals("FourteenAS"),
            "letter keys carry the keyboard's ids");
    }

    private static void nineKeyboard() {
        List<List<Key>> standard = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false);
        check(standard.size() == 4, "nine keys draw the 3×3 frame and the bottom row");
        List<Key> first = standard.get(0);
        check(first.size() == 5 && first.get(0).face().equals("，") && first.get(4).face().equals("⌫"),
            "symbol rail, three grid keys and ⌫");
        check(standard.get(1).get(4).face().equals("重输") && standard.get(1).get(4).tone() == Tone.FUNCTION,
            "重输 is a function key");
        check(faces(standard.get(3)).equals(List.of("符", "123", "空格", "中", "↵")), "standard nine-key bottom row");
        for (Row style : List.of(Row.STANDARD, Row.COMMA_PERIOD, Row.EMOJI)) {
            sameEntries(KeyLayoutStylesPolicy.keyboard(Kind.NINE, style, false, false).get(3),
                KeyboardActionRow.designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false, false, false, false, style),
                "nine-key bottom row " + style);
        }
        List<List<Key>> tall = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.TALL_RETURN, false, false);
        Key enter = tall.get(2).get(tall.get(2).size() - 1);
        check(enter.id().equals("Enter") && enter.rows() == 2 && enter.tone() == Tone.RETURN, "the tall return spans the last two rows");
        List<Key> bottom = tall.get(3);
        check(bottom.get(0).face().equals("符") && bottom.get(bottom.size() - 1).rows() == 0,
            "符 sits under the rail and the return covers the bottom-right cell");
        check(near(sum(bottom), sum(tall.get(2))), "the tall frame's rows share one width");
    }

    private static List<String> labels(Kind kind) {
        List<String> labels = new ArrayList<>();
        for (Row row : kind.rows) labels.add(KeyLayoutStylesPolicy.label(kind, row));
        return labels;
    }

    private static List<String> faces(List<Key> row) {
        List<String> faces = new ArrayList<>(row.size());
        for (Key key : row) faces.add(key.face());
        return faces;
    }

    private static float sum(List<Key> row) {
        float total = 0;
        for (Key key : row) total += key.weight();
        return total;
    }

    private static void sameEntries(List<Key> row, List<KeyboardActionRow.DesignEntry> entries, String message) {
        check(row.size() == entries.size(), message + ": key count");
        for (int index = 0; index < row.size(); index++)
            check(near(row.get(index).weight(), entries.get(index).weight()), message + ": weight " + index);
    }

    private static boolean near(float left, float right) {
        return Math.abs(left - right) < 1e-4f;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
