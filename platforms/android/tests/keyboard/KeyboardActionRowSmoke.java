import app.msime.android.KeyboardActionRow;
import app.msime.android.KeyboardActionRow.Slot;
import app.msime.android.KeyboardLayout;
import java.util.List;

public final class KeyboardActionRowSmoke {
    public static void main(String[] args) {
        // 新设计底行：123 1.25 / 中 1.05 / ， 1 / 空格 4 / 。 1 / ↵ 1.9，地球键条件插入。
        List<KeyboardActionRow.DesignEntry> design =
            KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false);
        check(design.equals(List.of(
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.LAYER, 1.25f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.LANGUAGE, 1.05f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.COMMA, 1f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.SPACE, 4f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.PERIOD, 1f),
                new KeyboardActionRow.DesignEntry(KeyboardActionRow.DesignSlot.RETURN, 1.9f))),
            "design row order and weights");
        List<KeyboardActionRow.DesignEntry> withGlobe =
            KeyboardActionRow.designEntries(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true);
        check(withGlobe.size() == 7 && withGlobe.get(2).slot() == KeyboardActionRow.DesignSlot.GLOBE,
            "the globe follows 中/英 only when switching is offered");
        check(KeyboardActionRow.designEntries(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, true).isEmpty(),
            "the kana grid takes no design row either");
        check(KeyboardActionRow.designEntries(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false).stream()
                .noneMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.COMMA
                    || entry.slot() == KeyboardActionRow.DesignSlot.PERIOD),
            "nine-key keeps punctuation in its sidebar");
        check(KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false).isEmpty()
                && KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true).isEmpty(),
            "the Zhuyin nine-key block carries 123, language, space and return itself, so there is no design row");
        check("，".equals(KeyboardActionRow.punctuationFace(KeyboardActionRow.DesignSlot.COMMA, true))
            && ".".equals(KeyboardActionRow.punctuationFace(KeyboardActionRow.DesignSlot.PERIOD, false)),
            "comma and period faces follow Chinese punctuation");
        check("切换到数字和符号".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.LAYER, true))
            && "切换中英文".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.LANGUAGE, true))
            && "切换输入法".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.GLOBE, true))
            && "空格".equals(KeyboardActionRow.designDescription(KeyboardActionRow.DesignSlot.SPACE, true)),
            "design row descriptions follow §2.8");

        check(KeyboardActionRow.entries(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, true).isEmpty(),
            "the kana grid carries its own side columns and takes no action row");
        check(slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true).equals(List.of(
                Slot.SYMBOL_PANEL, Slot.LAYER, Slot.GLOBE, Slot.PUNCTUATION, Slot.SPACE,
                Slot.LANGUAGE, Slot.RETURN)),
            "standard row order");
        check(!slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false).contains(Slot.GLOBE),
            "no globe when the host cannot switch input methods");
        check(!slots(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true).contains(Slot.PUNCTUATION),
            "the nine-key sidebar already carries punctuation");
        check(!slots(KeyboardLayout.STROKE_LAYOUT, true).contains(Slot.PUNCTUATION),
            "the stroke keypad's sidebar carries punctuation as nine-key's does");
        check(slots(KeyboardLayout.HANDWRITING_LAYOUT, true).contains(Slot.PUNCTUATION),
            "handwriting keeps the quick punctuation key");
        check(slots(KeyboardLayout.KOREAN_LAYOUT, true).equals(
                slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the Korean keycaps sit over the standard rows and keep the standard action row");
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT,
                KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, KeyboardLayout.HANDWRITING_LAYOUT,
                KeyboardLayout.KOREAN_LAYOUT, KeyboardLayout.ZHUYIN_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
                KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT}) {
            List<Slot> slots = slots(layout, true);
            check(slots.contains(Slot.SPACE) && slots.contains(Slot.RETURN)
                && slots.contains(Slot.LANGUAGE), "every action row commits, spaces and switches");
            check(slots.size() == slots.stream().distinct().count(), "no slot appears twice");
            float total = 0;
            for (KeyboardActionRow.Entry entry : KeyboardActionRow.entries(layout, true))
                total += entry.weight();
            check(total > 0, "weights are positive");
        }
        check(weight(KeyboardLayout.STANDARD_TOUCH_LAYOUT, Slot.SPACE)
            > weight(KeyboardLayout.STANDARD_TOUCH_LAYOUT, Slot.RETURN),
            "space is the widest key in the row");

        // Delete and 大小写 live in the last letter row whenever those rows are the surface, so the
        // action row must not repeat them.
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false),
            "letter rows end with delete");
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true),
            "symbol rows end with delete");
        check(!KeyboardActionRow.rowsCarryDelete(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false),
            "the nine-key grid owns its delete key");
        check(!KeyboardActionRow.rowsCarryDelete(KeyboardLayout.HANDWRITING_LAYOUT, false),
            "the handwriting tool column owns its delete key");
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.HANDWRITING_LAYOUT, true),
            "handwriting hands its symbol layer to the letter rows");
        check(KeyboardActionRow.rowsCarryCase(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false),
            "letter rows start with the case key");
        check(!KeyboardActionRow.rowsCarryCase(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true),
            "the digit page has no case to shift");
        check(!KeyboardActionRow.rowsCarryCase(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, false),
            "九键切数字仍然是九键，没有大小写可切");
        check(KeyboardActionRow.rowsCarryCase(KeyboardLayout.KOREAN_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.KOREAN_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.KOREAN_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.KOREAN_LAYOUT, true),
            "the Korean rows carry Shift for the double consonants and delete, like the standard rows");
        check(KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_LAYOUT, false)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_LAYOUT, true),
            "the Dachen rows carry delete but no case key: bopomofo has no case");
        check(!KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STROKE_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.STROKE_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.STROKE_LAYOUT, false)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.STROKE_LAYOUT, true),
            "the stroke grid owns its delete key and hands its symbol layer to the letter rows, with no case key");
        check(slots(KeyboardLayout.STROKE_LAYOUT, true).equals(
                slots(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true)),
            "the stroke keypad keeps the nine-key action row");
        check(slots(KeyboardLayout.ZHUYIN_LAYOUT, true).equals(
                slots(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the Dachen rows keep the standard action row");
        // 注音 9 键的网格自带 @#、逗号句号和 ⌫，底排不再重复它们。
        check(slots(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true).equals(List.of(
                Slot.LAYER, Slot.GLOBE, Slot.SPACE, Slot.LANGUAGE, Slot.RETURN))
                && slots(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false).equals(List.of(
                Slot.LAYER, Slot.SPACE, Slot.LANGUAGE, Slot.RETURN)),
            "the Zhuyin nine-key grid carries the symbol panel key and punctuation itself");
        check(!KeyboardActionRow.usesLetterRows(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false)
                && KeyboardActionRow.usesLetterRows(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false)
                && KeyboardActionRow.rowsCarryDelete(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false)
                && !KeyboardActionRow.rowsCarryCase(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true),
            "the Zhuyin nine-key grid owns its delete key and hands its symbol layer to the letter rows, with no case key");
        check(KeyboardActionRow.letterRowEdgeWeight(false)
            > KeyboardActionRow.letterRowEdgeWeight(true),
            "the ten-key symbol row leaves its edges less room than the seven-key letter row");

        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, false)),
            "letters offer the digit page");
        check("ABC".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STANDARD_TOUCH_LAYOUT, true)),
            "the digit page returns to 26 keys");
        check("九键".equals(KeyboardActionRow.layerTitle(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true)),
            "the digit page returns to the grid the user picked");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.KOREAN_LAYOUT, false))
                && "한".equals(KeyboardActionRow.layerTitle(KeyboardLayout.KOREAN_LAYOUT, true)),
            "the Korean digit page returns to the Hangul keycaps");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_LAYOUT, false))
                && "注".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_LAYOUT, true)),
            "the Zhuyin digit page returns to the Dachen keycaps");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false))
                && "注".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, true)),
            "the Zhuyin nine-key digit page returns to the bopomofo grid");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STROKE_LAYOUT, false))
                && "笔".equals(KeyboardActionRow.layerTitle(KeyboardLayout.STROKE_LAYOUT, true)),
            "the stroke digit page returns to the stroke keypad");
        check("切换到数字和符号".equals(KeyboardActionRow.layerDescription(false))
            && "切换到字母键盘".equals(KeyboardActionRow.layerDescription(true)),
            "layer key descriptions");
        System.out.println("Android action row: composition, weights and layer faces passed");
    }

    private static List<Slot> slots(int layout, boolean globe) {
        return KeyboardActionRow.entries(layout, globe).stream()
            .map(KeyboardActionRow.Entry::slot).toList();
    }

    private static float weight(int layout, Slot slot) {
        for (KeyboardActionRow.Entry entry : KeyboardActionRow.entries(layout, true))
            if (entry.slot() == slot) return entry.weight();
        throw new AssertionError("Missing slot: " + slot);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
