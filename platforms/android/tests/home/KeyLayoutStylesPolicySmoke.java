import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.KeyboardScheme;
import app.msime.android.NineKeySidebarPolicy;
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
 * 「按键布局」子页的规则（与 iOS、共享设置页同一份规格）：分段列哪几类、默认停在哪一类，每类的胶囊名字和顺序、选样式写哪一项偏好，高亮集合，说明的显示与隐藏，数字行只画在 26 和 14/18 键上，整块预览的键序、份额和跨行取自键盘的表，「26 键「符」与「123」互换」打开后预览底行 符 在 123 左边并高亮这两键，「中/英键放在空格左边」打开后三类预览底行的中/英排到空格（和贴着空格的标点）左边并高亮它，「数字键盘」一组胶囊的名字、顺序和写法，以及两行左侧符号的标题、摘要、编辑页说明和 9 键预览的左列（#10223）。读写偏好和画图的那一半在 `KeyLayoutStylesPage`、`KeyLayoutPreview` 里，要设备，这里跑不了。
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
        symbolFirst();
        languageLeft();
        numberPadSpace();
        nineKeySidebar();
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
        check(labels(Kind.TWENTY_SIX).equals(List.of("标准", "逗号句号", "长空格", "表情键")), "26-key chips");
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

    private static void symbolFirst() {
        check(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE.equals("touch_twenty_six_symbol_first"), "the swap writes its shared preference");
        // 打开后 26 键有 符 的样式（标准、逗号句号）把 符 排到 123 左边，份额与键盘按同一条规则排的底行相同。
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false, true).get(3))
            .equals(List.of("符", "123", "空格", "，", "中", "↵")), "standard bottom row puts 符 before 123");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.COMMA_PERIOD, false, false, true).get(3)).subList(0, 2)
            .equals(List.of("符", "123")), "comma-period bottom row puts 符 before 123");
        for (Row style : Kind.TWENTY_SIX.rows) {
            sameEntries(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, style, true, false, true).get(4),
                KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false, false, false, false, style, true,
                    TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT), "swapped 26-key bottom row " + style);
        }
        // 长空格和表情键样式没有 符，打开与否预览一样，也不多高亮。
        for (Row style : List.of(Row.LONG_SPACE, Row.EMOJI)) {
            check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, style, false, false, true).get(3))
                .equals(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, style, false, false).get(3))), style + " has no 符 to move");
            check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, style, true).equals(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, style)),
                style + " adds no swap highlight");
        }
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.STANDARD, true).equals(Set.of("SoftSymbol", "SoftLayer")),
            "the swap highlights 符 and 123 on the standard row");
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.COMMA_PERIOD, true)
            .equals(Set.of("Comma", "Period", "SoftSymbol", "SoftLayer")), "the swap adds 符 and 123 to the style's highlights");
        check(KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, Row.STANDARD, false).isEmpty(), "off, standard still highlights nothing");
        // 互换只作用于 26 键，14/18 键和 9 键的预览与高亮不变。
        for (Kind kind : List.of(Kind.FOURTEEN, Kind.NINE)) {
            for (Row style : kind.rows) {
                check(KeyLayoutStylesPolicy.highlights(kind, style, true).equals(KeyLayoutStylesPolicy.highlights(kind, style)),
                    kind + " " + style + " ignores the swap highlight");
                check(faces(last(KeyLayoutStylesPolicy.keyboard(kind, style, false, false, true)))
                    .equals(faces(last(KeyLayoutStylesPolicy.keyboard(kind, style, false, false)))), kind + " " + style + " ignores the swap");
            }
        }
        for (Row style : Kind.TWENTY_SIX.rows) {
            List<String> drawn = new ArrayList<>();
            for (List<Key> keys : KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, style, false, false, true)) {
                for (Key key : keys) if (key.rows() > 0) drawn.add(key.id());
            }
            for (String id : KeyLayoutStylesPolicy.highlights(Kind.TWENTY_SIX, style, true))
                check(drawn.contains(id), "swapped " + style + " draws its highlighted " + id);
        }
    }

    /** 「中/英键放在空格左边」（#10228，`touch_language_key_side`）：三个分段的预览底行都按键盘的同一条规则重排（{@link KeyboardActionRow#languageBeforeSpace}），与互换叠加；打开时在样式和互换的高亮之外只多高亮中/英，关上时预览与高亮和原来逐键相同。 */
    private static void languageLeft() {
        check(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE.equals("touch_language_key_side")
            && TouchKeyStyle.languageKeySideWire(true).equals("left") && TouchKeyStyle.languageKeySideWire(false).equals("right"),
            "the toggle writes left when on and right when off");
        check(TouchKeyStyle.LANGUAGE_LEFT_TITLE.equals("中/英键放在空格左边")
            && TouchKeyStyle.LANGUAGE_LEFT_NOTE.equals("中/英 排到空格左边，逗号、句号仍紧挨空格。"), "toggle title and note");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false, false, true).get(3))
            .equals(List.of("123", "符", "中", "空格", "，", "↵")), "26 standard puts 中 before the space");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.COMMA_PERIOD, false, false, false, true).get(3))
            .equals(List.of("123", "符", "中", "，", "空格", "。", "↵")), "26 comma-period keeps ， against the space");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.LONG_SPACE, false, false, false, true).get(3))
            .equals(List.of("123", "中", "，", "空格", "↵")), "26 long space");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.EMOJI, false, false, false, true).get(3))
            .equals(List.of("123", "😀", "中", "空格", "，", "↵")), "26 emoji");
        check(faces(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, Row.STANDARD, false, false, true, true).get(3))
            .equals(List.of("符", "123", "中", "空格", "，", "↵")), "the swap and 中 on the left combine");
        check(faces(last(KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, Row.STANDARD, false, false, false, true)))
            .equals(List.of("123", "中", "，", "空格", "。", "↵")), "14/18 standard");
        check(faces(last(KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false, false, true)))
            .equals(List.of("符", "123", "中", "空格", "↵")), "9 standard");
        check(faces(last(KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.COMMA_PERIOD, false, false, false, true)))
            .equals(List.of("符", "123", "中", "，", "空格", "。", "↵")), "9 bottom-bar punctuation");
        List<Key> tall = last(KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.TALL_RETURN, false, false, false, true));
        check(faces(tall).subList(0, 4).equals(List.of("符", "123", "中", "空格")) && tall.get(tall.size() - 1).rows() == 0,
            "tall return: 123 | 中 | 空格 under the grid, the return still covers the bottom-right cell");
        for (Kind kind : Kind.values()) {
            for (Row style : kind.rows) {
                for (boolean symbolFirst : new boolean[] {false, true}) {
                    List<List<Key>> off = KeyLayoutStylesPolicy.keyboard(kind, style, true, false, symbolFirst, false);
                    List<List<Key>> on = KeyLayoutStylesPolicy.keyboard(kind, style, true, false, symbolFirst, true);
                    String label = kind + " " + style + " symbolFirst " + symbolFirst;
                    check(off.equals(KeyLayoutStylesPolicy.keyboard(kind, style, true, false, symbolFirst)), label + ": off is today's preview");
                    check(on.size() == off.size() && on.subList(0, on.size() - 1).equals(off.subList(0, off.size() - 1)),
                        label + ": only the bottom row changes");
                    check(near(sum(last(on)), sum(last(off))) && last(on).containsAll(last(off)), label + ": same keys, same widths");
                    Set<String> highlighted = KeyLayoutStylesPolicy.highlights(kind, style, symbolFirst, true);
                    Set<String> without = KeyLayoutStylesPolicy.highlights(kind, style, symbolFirst);
                    check(KeyLayoutStylesPolicy.highlights(kind, style, symbolFirst, false).equals(without), label + ": off adds no highlight");
                    check(highlighted.contains("SoftLanguage") && highlighted.containsAll(without) && highlighted.size() == without.size() + 1,
                        label + ": on adds only 中/英 to the highlights");
                    List<String> drawn = new ArrayList<>();
                    for (Key key : last(on)) if (key.rows() > 0) drawn.add(key.id());
                    check(drawn.contains("SoftLanguage"), label + ": the highlighted 中/英 is drawn in the bottom row");
                }
            }
        }
        for (Row style : Kind.TWENTY_SIX.rows) {
            sameEntries(last(KeyLayoutStylesPolicy.keyboard(Kind.TWENTY_SIX, style, false, false, true, true)),
                KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false, false, false, false, style, true,
                    TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, true), "26-key preview follows the keyboard " + style);
        }
        for (Row style : Kind.FOURTEEN.rows) {
            sameEntries(last(KeyLayoutStylesPolicy.keyboard(Kind.FOURTEEN, style, false, false, false, true)),
                KeyboardActionRow.designEntries(KeyboardLayout.FOURTEEN_KEY_LAYOUT, false, false, false, false, style, false,
                    TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, true), "14/18 preview follows the keyboard " + style);
        }
        for (Row style : List.of(Row.STANDARD, Row.COMMA_PERIOD, Row.EMOJI)) {
            sameEntries(last(KeyLayoutStylesPolicy.keyboard(Kind.NINE, style, false, false, false, true)),
                KeyboardActionRow.designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false, false, false, false, style, false,
                    TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT, true), "9 preview follows the keyboard " + style);
        }
    }

    private static void numberPadSpace() {
        // 「数字键盘」一组的胶囊：三种排法按这个次序，点哪一枚写哪一枚的写法；没选过时选中 Android 原来的「空格在右列」。
        check(TouchKeyStyle.NUMBER_PAD_SPACE_PREFERENCE.equals("touch_number_pad_space"), "number pad chips write their shared preference");
        List<String> titles = new ArrayList<>();
        List<String> wires = new ArrayList<>();
        for (TouchKeyStyle.NumberPadSpace choice : TouchKeyStyle.NumberPadSpace.CHOICES) {
            titles.add(choice.title);
            wires.add(choice.wire);
        }
        check(titles.equals(List.of("空格在右列", "空格在 0 右边", "空格在 0 左边")), "number pad chip titles");
        check(wires.equals(List.of("right_column", "after_zero", "before_zero")), "number pad chip wires");
        check(TouchKeyStyle.NumberPadSpace.parse(null).resolved() == TouchKeyStyle.NumberPadSpace.RIGHT_COLUMN,
            "an unset number pad selects 空格在右列");
        check(TouchKeyStyle.NUMBER_PAD_SPACE_GROUP.equals("数字键盘")
            && TouchKeyStyle.NUMBER_PAD_SPACE_NOTE.equals("九宫格数字键盘里空格的位置；右侧大回车不受影响。"), "number pad group title and note");
    }

    private static void nineKeySidebar() {
        check(KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_TITLE.equals("九键左侧符号")
            && KeyLayoutStylesPolicy.DIGIT_SIDEBAR_TITLE.equals("数字键盘左侧符号"), "row and editor titles");
        check(KeyLayoutStylesPolicy.NINE_KEY_SIDEBAR_NOTE.equals("拼音九键、九键双拼和笔画共用，多于一屏时上下滑动。")
            && KeyLayoutStylesPolicy.DIGIT_SIDEBAR_NOTE.equals("所有九宫格数字键盘共用，#+= 固定在最后。"), "editor notes");
        List<String> letters = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        List<String> digits = NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS;
        // 摘要：空表显示出厂表；不超过四项时全列，多于四项时前四项加「 等 N 个」。
        check(KeyLayoutStylesPolicy.sidebarSummary(List.of(), letters).equals("， 。 ？"), "an empty letter table summarises the defaults");
        check(KeyLayoutStylesPolicy.sidebarSummary(List.of(), digits).equals("+ - * / 等 14 个"), "the digit defaults are summarised");
        check(KeyLayoutStylesPolicy.sidebarSummary(List.of("，", "。", "？", "！", "、", "；", "：", "…"), letters)
            .equals("， 。 ？ ！ 等 8 个"), "a long custom table is summarised");
        check(KeyLayoutStylesPolicy.sidebarSummary(List.of("、", "……"), letters).equals("、 ……"), "a short custom table is listed whole");

        // 出厂表、非右侧大回车：与 #10208 相同，左列逐行画 ， 。 ？，不高亮。
        List<List<Key>> standard = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false, false, List.of());
        check(standard.equals(KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false)), "the factory table draws as before");
        check(standard.equals(KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false, false, letters)),
            "a table equal to the factory table draws as the factory table");
        check(KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.STANDARD, false, List.of()).isEmpty()
            && KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.STANDARD, false, letters).isEmpty(), "the factory table is not highlighted");

        // 自定义表：左列是一整条跨三行的栏，一屏最多五项、等分，整条描强调色；右列的 ！ 不跟着高亮。
        List<String> custom = List.of("、", "……", "：", "；", "～", "·", "（）");
        List<List<Key>> drawn = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false, false, custom);
        Key rail = drawn.get(0).get(0);
        check(rail.rows() == 3 && rail.stack().equals(custom.subList(0, NineKeySidebarPolicy.VISIBLE_ROWS)),
            "the custom rail spans the grid and shows one screen");
        check(drawn.get(1).get(0).rows() == 0 && drawn.get(2).get(0).rows() == 0, "the rows below only keep the rail's width");
        for (int index = 0; index < 3; index++)
            check(near(drawn.get(index).get(0).weight(), standard.get(index).get(0).weight())
                && faces(drawn.get(index)).subList(1, 5).equals(faces(standard.get(index)).subList(1, 5)),
                "the grid and right column do not move on row " + index);
        check(faces(drawn.get(3)).equals(faces(standard.get(3))), "the bottom row does not change");
        Set<String> marked = KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.STANDARD, false, custom);
        check(marked.contains(rail.id()) && marked.size() == 1, "only the rail is highlighted");
        check(!marked.contains(standard.get(2).get(4).id()), "the right-column ！ is not highlighted with it");
        Key single = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.STANDARD, false, false, false, List.of("、")).get(0).get(0);
        check(single.stack().equals(List.of("、")), "a single symbol fills the rail");
        check(KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.COMMA_PERIOD, false, custom).equals(Set.of("Comma", "Period", rail.id())),
            "the rail adds to the style's own highlights");

        // 右侧大回车：出厂表接 ！ 共四项、不高亮；自定义表原样，不接 ！。
        List<List<Key>> tall = KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.TALL_RETURN, false, false, false, List.of());
        check(tall.get(0).get(0).stack().equals(NineKeySidebarPolicy.TALL_RETURN_LETTER_SYMBOLS), "the tall frame draws ， 。 ？ ！");
        check(KeyLayoutStylesPolicy.highlights(Kind.NINE, Row.TALL_RETURN, false, List.of()).equals(Set.of("Enter")),
            "the tall factory rail is not highlighted");
        check(KeyLayoutStylesPolicy.keyboard(Kind.NINE, Row.TALL_RETURN, false, false, false, List.of("、", "……")).get(0).get(0)
            .stack().equals(List.of("、", "……")), "a custom tall rail gets no ！");
        check(tall.get(3).get(0).face().equals("符"), "符 stays under the rail");

        // 字母表只影响 9 键：26 键、14/18 键的预览和高亮不变。
        for (Kind kind : List.of(Kind.TWENTY_SIX, Kind.FOURTEEN)) {
            check(KeyLayoutStylesPolicy.keyboard(kind, Row.STANDARD, false, false, false, custom)
                .equals(KeyLayoutStylesPolicy.keyboard(kind, Row.STANDARD, false, false)), kind + " ignores the letter table");
            check(KeyLayoutStylesPolicy.highlights(kind, Row.STANDARD, false, custom).isEmpty(), kind + " highlights nothing for it");
        }
    }

    private static List<Key> last(List<List<Key>> rows) {
        return rows.get(rows.size() - 1);
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
