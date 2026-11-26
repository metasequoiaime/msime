package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 键面会变的键各自的 {@link KeyFaceSpec}：键上画的字只写在这里，内容域、建键时的文字和渲染时设上的文字都由它们得出。键会出现在哪些布局上由 {@link #on} 判断，不在的布局上不进内容域。
 */
public final class KeyFaceSpecs {
    private KeyFaceSpecs() { }

    /** 26 键的 ⇧：平时画 ⇧ 或大写锁定的图标；26 键全拼组字可用分词时（{@link KeyboardLayout#usesQuanpinSeparator}）写「分词」，「全拼 Shift 键切大小写」开着时不换。 */
    public enum Shift { SHIFT, CAPS_LOCK, SEPARATOR }

    /** 26 键全拼的 ⇧ 换成的字。 */
    public static final String SEPARATOR_FACE = "分词";

    public static final KeyFaceSpec<Shift> SHIFT = new KeyFaceSpec<>() {
        @Override public List<Shift> states(KeyFaceLayout layout) {
            return separates(layout) ? List.of(Shift.SHIFT, Shift.CAPS_LOCK, Shift.SEPARATOR)
                : List.of(Shift.SHIFT, Shift.CAPS_LOCK);
        }

        @Override public Shift state(KeyFaceLayout layout, KeyFaceLive live) {
            if (separates(layout) && KeyboardLayout.usesQuanpinSeparator(live.selected() == KeyboardScheme.QUANPIN,
                    live.chineseSession(), live.symbols() ? KeyboardLayout.Layer.SYMBOLS : KeyboardLayout.Layer.LETTERS,
                    layout.touchLayout(), live.localModeNone() ? "none" : "temporary"))
                return Shift.SEPARATOR;
            return live.capsLock() ? Shift.CAPS_LOCK : Shift.SHIFT;
        }

        @Override public KeyFace face(Shift state) {
            return switch (state) {
                case SHIFT -> KeyFace.icon(KeyboardIconKey.Kind.SHIFT);
                case CAPS_LOCK -> KeyFace.icon(KeyboardIconKey.Kind.CAPS_LOCK);
                case SEPARATOR -> KeyFace.text(SEPARATOR_FACE);
            };
        }

        private boolean separates(KeyFaceLayout layout) {
            return layout.touchLayout() == KeyboardLayout.STANDARD_TOUCH_LAYOUT && !layout.quanpinShift();
        }
    };

    /** 14/18 键第三排左端的键：空闲时「符」、拼音组字时「分词」（{@link FourteenKeyLayout#separatorFace}），能用网格辅助码时右上角标「辅」。 */
    public record Separator(boolean composing, boolean helpcode) { }

    public static final KeyFaceSpec<Separator> FOURTEEN_SEPARATOR = new KeyFaceSpec<>() {
        @Override public List<Separator> states(KeyFaceLayout layout) {
            return List.of(new Separator(false, false), new Separator(false, true), new Separator(true, false),
                new Separator(true, true));
        }

        @Override public Separator state(KeyFaceLayout layout, KeyFaceLive live) {
            return new Separator(FourteenKeyLayout.separatesSyllables(live.composing(), live.engineScheme()), live.helpcode());
        }

        @Override public KeyFace face(Separator state) {
            return KeyFace.text(FourteenKeyLayout.separatorFace(state.composing()))
                .withCorner(GridHelpcodePolicy.cornerHint(state.helpcode()));
        }

        @Override public boolean on(KeyFaceLayout layout) {
            return layout.touchLayout() == KeyboardLayout.FOURTEEN_KEY_LAYOUT
                || layout.touchLayout() == KeyboardLayout.EIGHTEEN_KEY_LAYOUT;
        }

        @Override public boolean drawsCorner() { return true; }
    };

    /** 拼音九键网格左上角的 1 键（{@link NineKeyLayout#face(NineKeyLayout.Key, boolean, boolean, String)}）：字母层空闲时「@#」，组字时按设置是「分词」或「重输」，数字层是「1」。 */
    public static final KeyFaceSpec<String> NINE_ONE = new TextSpec() {
        @Override public List<String> states(KeyFaceLayout layout) {
            NineKeyLayout.Key key = key();
            return distinct(NineKeyLayout.face(key, false), NineKeyLayout.face(key, false, true, layout.nineKeyAction()),
                NineKeyLayout.face(key, true));
        }

        @Override public String state(KeyFaceLayout layout, KeyFaceLive live) {
            return NineKeyLayout.face(key(), live.symbols(), live.composing(), layout.nineKeyAction());
        }

        @Override public boolean on(KeyFaceLayout layout) {
            return layout.touchLayout() == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT;
        }

        private NineKeyLayout.Key key() {
            return NineKeyLayout.rows().get(0).get(0);
        }
    };

    /** 日语九键左列切层的键：假名层「123」，符号层「あいう」。 */
    public static final KeyFaceSpec<String> JAPANESE_LAYER = new TextSpec() {
        @Override public List<String> states(KeyFaceLayout layout) {
            return List.of("123", JapaneseNineKeyLayout.LAYER_TITLE);
        }

        @Override public String state(KeyFaceLayout layout, KeyFaceLive live) {
            return states(layout).get(live.symbols() ? 1 : 0);
        }

        @Override public boolean on(KeyFaceLayout layout) { return layout.japaneseNineKey(); }
    };

    /** 日语九键网格左下角：假名层「小゛゜」，符号层「（）」。 */
    public static final KeyFaceSpec<String> JAPANESE_VARIANTS = new TextSpec() {
        @Override public List<String> states(KeyFaceLayout layout) {
            return List.of("小゛゜", "（）");
        }

        @Override public String state(KeyFaceLayout layout, KeyFaceLive live) {
            return states(layout).get(live.symbols() ? 1 : 0);
        }

        @Override public boolean on(KeyFaceLayout layout) { return layout.japaneseNineKey(); }
    };

    /** 日语九键右列的空格：「空白」，组字时「変換」（{@link JapaneseNineKeyActions#spaceTitle}）。 */
    public static final KeyFaceSpec<String> JAPANESE_SPACE = new TextSpec() {
        @Override public List<String> states(KeyFaceLayout layout) {
            return List.of(JapaneseNineKeyActions.spaceTitle(false), JapaneseNineKeyActions.spaceTitle(true));
        }

        @Override public String state(KeyFaceLayout layout, KeyFaceLive live) {
            return JapaneseNineKeyActions.spaceTitle(live.composing());
        }

        @Override public boolean on(KeyFaceLayout layout) { return layout.japaneseNineKey(); }
    };

    /** 语言键（日语九键左列；底行的中/英随底行迁入）：「中」「英」，打开「中英键轮换其他语言」时还有可选方案里其他语言的字（{@link LanguageKeyCyclePolicy#label}）。 */
    public static final KeyFaceSpec<String> LANGUAGE = new TextSpec() {
        @Override public List<String> states(KeyFaceLayout layout) {
            List<String> faces = new ArrayList<>(List.of("中", "英"));
            if (!layout.languageCycles()) return faces;
            for (KeyboardScheme scheme : layout.schemes())
                if (LanguageKeyCyclePolicy.label(true, scheme, false, layout.schemes()).equals(scheme.glyph())
                        && !faces.contains(scheme.glyph())) faces.add(scheme.glyph());
            return faces;
        }

        @Override public String state(KeyFaceLayout layout, KeyFaceLive live) {
            return LanguageKeyCyclePolicy.label(layout.languageCycles(), live.selected(), live.english(), layout.schemes());
        }

        @Override public boolean on(KeyFaceLayout layout) { return layout.japaneseNineKey(); }
    };

    /** 键区功能键组在 {@code layout} 上能画的文字（{@code slot} 位置）：⇧、14/18 键的分词键，日语九键再加两侧列和左下角会变的文字键。⌫ 等图标不占文字；「重输」「符」「切换」这类文字固定的键放进组时自己的文字并进来（{@link KeyFaceGroup#attach}）。 */
    public static List<String> functionTexts(KeyFaceLayout layout, KeyFaceSlot slot) {
        List<String> texts = new ArrayList<>();
        for (KeyFaceSpec<?> spec : List.of(SHIFT, FOURTEEN_SEPARATOR, JAPANESE_LAYER, JAPANESE_VARIANTS,
                JAPANESE_SPACE, LANGUAGE)) {
            if (!spec.on(layout)) continue;
            for (String text : spec.texts(layout, slot)) if (!texts.contains(text)) texts.add(text);
        }
        return texts;
    }

    /** 键区功能键组在 {@code layout} 上的内容域：主标签和右上角各按 {@link #functionTexts}。 */
    public static KeyFaceDomain functionDomain(KeyFaceLayout layout) {
        java.util.Map<KeyFaceSlot, List<String>> texts = new java.util.EnumMap<>(KeyFaceSlot.class);
        for (KeyFaceSlot slot : new KeyFaceSlot[] {KeyFaceSlot.MAIN, KeyFaceSlot.TOP_RIGHT})
            texts.put(slot, functionTexts(layout, slot));
        return new KeyFaceDomain(texts);
    }

    /** 状态就是键上那段字的键。 */
    private abstract static class TextSpec implements KeyFaceSpec<String> {
        @Override public KeyFace face(String state) {
            return KeyFace.text(state);
        }
    }

    private static List<String> distinct(String... texts) {
        List<String> list = new ArrayList<>();
        for (String text : texts) if (!list.contains(text)) list.add(text);
        return list;
    }
}
