import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardLayout;
import app.msime.android.StrokeKeyboardLayout;
import java.util.ArrayList;
import java.util.List;

/** 笔画键盘：2×3 网格的字形、名称与发送的字母，以及空组合时的通配键。 */
public final class StrokeKeyboardLayoutSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        List<List<StrokeKeyboardLayout.Key>> rows = StrokeKeyboardLayout.rows();
        check(rows.size() == 2 && rows.get(0).size() == 3 && rows.get(1).size() == 3, "the grid is 2 by 3");
        List<String> glyphs = new ArrayList<>();
        List<String> names = new ArrayList<>();
        StringBuilder inputs = new StringBuilder();
        for (List<StrokeKeyboardLayout.Key> row : rows) {
            for (StrokeKeyboardLayout.Key key : row) {
                glyphs.add(key.glyph());
                names.add(key.name());
                inputs.append(key.input());
            }
        }
        check(glyphs.equals(List.of("一", "丨", "丿", "丶", "乛", "＊")), "the faces are 一丨丿丶乛＊: " + glyphs);
        check(names.equals(List.of("横", "竖", "撇", "点", "折", "通配")), "the names are 横竖撇点折通配: " + names);
        check("hspnzx".contentEquals(inputs), "the keys send the stroke letters h s p n z and the wildcard x: " + inputs);
        // Every face is one BMP character, so an older Android font never draws a surrogate pair as tofu.
        for (String glyph : glyphs) check(glyph.length() == 1, "one UTF-16 unit per face: " + glyph);
        check(StrokeKeyboardLayout.WILDCARD == 'x', "the wildcard is x");

        StrokeKeyboardLayout.Key horizontal = rows.get(0).get(0);
        StrokeKeyboardLayout.Key wildcard = rows.get(1).get(2);
        check("一\n横".equals(StrokeKeyboardLayout.face(horizontal)), "a key prints its glyph over its name");
        check("笔画 横".equals(StrokeKeyboardLayout.accessibilityLabel(horizontal))
            && "笔画 通配".equals(StrokeKeyboardLayout.accessibilityLabel(wildcard)), "spoken labels name the stroke");

        // Idle, the Engine does not start a composition from x and the host would type it, so the keypad holds it back.
        check(!StrokeKeyboardLayout.sends(wildcard.input(), false), "the wildcard is off with nothing composing");
        check(StrokeKeyboardLayout.sends(wildcard.input(), true), "the wildcard extends a composition");
        for (char stroke : "hspnz".toCharArray()) {
            check(StrokeKeyboardLayout.sends(stroke, false) && StrokeKeyboardLayout.sends(stroke, true),
                "a stroke key always sends: " + stroke);
        }
        boolean rejected = false;
        try { StrokeKeyboardLayout.face(null); } catch (IllegalArgumentException expected) { rejected = true; }
        check(rejected, "a missing key is a programming error");

        // The surface: the scheme forces it over the stored 26-key or nine-key layout, handwriting keeps its panel, dedicated English is the host's standard rows.
        check(KeyboardLayout.STROKE_LAYOUT == 6, "the stroke surface constant");
        check(KeyboardLayout.resolveTouchLayout(false, false, 9, "twenty_six_key") == KeyboardLayout.STROKE_LAYOUT,
            "26-key preference shows the stroke keypad");
        check(KeyboardLayout.resolveTouchLayout(false, true, 9, "nine_key") == KeyboardLayout.STROKE_LAYOUT,
            "nine-key preference shows the stroke keypad");
        check(KeyboardLayout.resolveTouchLayout(true, false, 9, "handwriting") == KeyboardLayout.HANDWRITING_LAYOUT,
            "handwriting preference shows handwriting");
        check(!KeyboardLayout.carriesLetterCase(KeyboardLayout.STROKE_LAYOUT), "strokes have no case");
        check(KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.STROKE_LAYOUT)
            == KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS), "the symbol page is the shared one");

        // The chrome: the grid carries punctuation and delete; the symbol page hands over to the 26-key symbol rows as handwriting does.
        int stroke = KeyboardLayout.STROKE_LAYOUT;
        check(!KeyboardActionRow.usesLetterRows(stroke, false) && KeyboardActionRow.usesLetterRows(stroke, true),
            "the letter layer is the grid, the symbol layer the 26-key rows");
        check(!KeyboardActionRow.rowsCarryDelete(stroke, false) && KeyboardActionRow.rowsCarryDelete(stroke, true),
            "delete sits in the grid's right column, and in the symbol rows on that page");
        check(!KeyboardActionRow.rowsCarryCase(stroke, false) && !KeyboardActionRow.rowsCarryCase(stroke, true),
            "no case key");
        check(KeyboardActionRow.layerTitle(stroke, false).equals("123")
            && KeyboardActionRow.layerTitle(stroke, true).equals("笔")
            && StrokeKeyboardLayout.LAYER_TITLE.equals("笔"), "the symbol page returns to 笔");
        check(KeyboardActionRow.entries(stroke, true).stream()
            .noneMatch(entry -> entry.slot() == KeyboardActionRow.Slot.PUNCTUATION),
            "the bottom row matches nine-key: no punctuation slot beside the sidebar");
        check(KeyboardActionRow.entries(stroke, true).equals(
            KeyboardActionRow.entries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true))
            && KeyboardActionRow.entries(stroke, false).equals(
            KeyboardActionRow.entries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false)),
            "the bottom row is the nine-key one");
        System.out.println("Android stroke keyboard: 2x3 grid, faces, letters, idle wildcard, surface and chrome passed");
    }
}
