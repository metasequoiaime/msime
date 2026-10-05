package app.msime.android;

import java.util.List;

/** 空格键方案短名、新设计 123 层的适用界面与底行组成。 */
public final class ms_w3_kcb_KeyboardFacesSmoke {
    public static void main(String[] args) {
        check("全拼".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.QUANPIN, KeyboardScheme.WUBI_86, false)),
            "quanpin label");
        check("双拼 · 小鹤".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.XIAOHE, null, false)),
            "xiaohe label");
        check("五笔 86".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.WUBI, KeyboardScheme.WUBI_86, false)),
            "wubi 86 label");
        check("五笔 98".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.WUBI, KeyboardScheme.WUBI_98, false)),
            "wubi 98 label");
        check("笔画".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.STROKE, null, false)), "stroke label");
        check("日语".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.JAPANESE_NINE_KEY, null, false)),
            "japanese label");
        check("space".equals(SpaceKeyFace.schemeLabel(KeyboardScheme.XIAOHE, null, true)),
            "english mode shows space");
        check("全拼".equals(SpaceKeyFace.schemeLabel(null, null, false)), "missing scheme falls back");
        for (KeyboardScheme scheme : KeyboardScheme.values())
            check(!SpaceKeyFace.schemeLabel(scheme, null, false).isEmpty(), "label for " + scheme);

        check(ImeLetterRows.drawsDesignLayer(KeyboardLayout.STANDARD_TOUCH_LAYOUT), "26 keys draw the 123 layer");
        check(ImeLetterRows.drawsDesignLayer(KeyboardLayout.HANDWRITING_LAYOUT), "handwriting hands over");
        check(ImeLetterRows.drawsDesignLayer(KeyboardLayout.STROKE_LAYOUT), "stroke hands over");
        check(!ImeLetterRows.drawsDesignLayer(KeyboardLayout.ZHUYIN_LAYOUT), "dachen keeps its symbol rows");
        check(!ImeLetterRows.drawsDesignLayer(KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT), "nine-key keeps its grid");
        check(!ImeLetterRows.drawsDesignLayer(KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT), "kana keeps its grid");

        List<KeyboardActionRow.DesignEntry> row = KeyboardActionRow.designEntries(
            KeyboardLayout.STANDARD_TOUCH_LAYOUT, false);
        check(row.size() == 6 && row.get(0).slot() == KeyboardActionRow.DesignSlot.LAYER
            && row.get(5).slot() == KeyboardActionRow.DesignSlot.RETURN, "123 | 中 | ， | 空格 | 。 | ↵");
        List<KeyboardActionRow.DesignEntry> nine = KeyboardActionRow.designEntries(
            KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT, true);
        check(nine.stream().noneMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.COMMA),
            "nine-key keeps punctuation in its sidebar");
        check(nine.stream().anyMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.GLOBE),
            "globe only when offered");
        System.out.println("ms_w3_kcb_KeyboardFacesSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
