package app.msime.android;

import java.util.List;
import java.util.Set;

/**
 * 键组按作用分的角色：每个角色一行，写明它用哪个模板、键可能用到哪些提示位置、主标签以哪一组为上限（{@link KeyFaceGroup#capMainAt}），以及组里的键该不该一样大。一副键盘上一个角色一组，组里每个键用同一份参数，所以只有作用相同、键格一样大的键放进同一个角色；键宽各不相同、各按自己的字定字号的键（底行）另按每个键分组。
 *
 * <p>键格是否一样大由设备检查核对（组里最宽和最窄的键差不过一成五），有意宽窄不一的角色在这里写明原因（{@link #mixedWidths}）。
 */
public enum KeyFaceRole {
    /** 26 键（和同一套字母行的其他语言）字母层的字母键和微软双拼的 `;`。 */
    LETTERS(KeyFaceTemplates.LETTERS, Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM), null),
    /** 14 键的字母键。 */
    FOURTEEN_LETTERS(KeyFaceTemplates.GRID_LETTERS, Set.of(KeyFaceSlot.TOP_RIGHT),
        "两个字母的键和单字母键宽窄不同，字母键宽度还可按设置等宽或宽窄"),
    /** 18 键的字母键。 */
    EIGHTEEN_LETTERS(KeyFaceTemplates.GRID_LETTERS, Set.of(KeyFaceSlot.TOP_RIGHT),
        "两个字母的键和单字母键宽窄不同，字母键宽度还可按设置等宽或宽窄"),
    /** 字母上方的数字行。 */
    NUMBER_ROW(KeyFaceTemplates.NUMBER_ROW, Set.of(), null),
    /** 键区里的功能键：⇧、⌫、14/18 键的分词键、九键和笔画右列的重输，#+= 层自带的功能键。 */
    FUNCTIONS(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 日语九键的功能键：两侧列的文字键、左下角的「小゛゜」和 ⌫，字号和图标按侧列一格的高度定。 */
    JAPANESE_FUNCTIONS(KeyFaceTemplates.JAPANESE_FUNCTIONS, Set.of(),
        "两侧列五格分四行键高，网格左下角的「小゛゜」在网格里"),
    /** 拼音九键字母层的九格和右列的 ！。 */
    NINE_LETTERS(KeyFaceTemplates.NINE_LETTERS, Set.of(KeyFaceSlot.TOP_RIGHT), "右列的 ！ 在侧列，比网格的格子窄"),
    /** 数字九宫格的 1–9、右列的小数点和 ！、底行的 0。 */
    NINE_DIGITS(KeyFaceTemplates.NINE_DIGITS, Set.of(), "右列的小数点和 ！ 在侧列，底行的 0 在底行"),
    /** 九键双拼的数字键和底行的 0：底部是声韵提示。 */
    NINE_SHUANGPIN(KeyFaceTemplates.NINE_DIGITS, Set.of(KeyFaceSlot.BOTTOM), "底行的 0 在底行"),
    /** 韩文字母层的字母键。 */
    KOREAN(KeyFaceTemplates.KOREAN, Set.of(), "第二排九个键不像 26 键那样两端缩进半格，铺满一行，比第一排的键宽一成多"),
    /** 大千字母层的注音符号和声调键。 */
    DACHEN(KeyFaceTemplates.DACHEN, Set.of(), null),
    /** 日文罗马字的长音键 ー：自己一组，全角的 ー 放进字母组会让窄键上的每个字母都按它缩小。 */
    LONG_VOWEL(KeyFaceTemplates.LETTERS, Set.of(), null),
    /** 笔画键盘的笔画键和通配键。 */
    STROKES(KeyFaceTemplates.STROKES, Set.of(), null),
    /** 笔画键盘网格最后一行的 @#、0、句点和右列的 ！。 */
    STROKE_KEYS(KeyFaceTemplates.STROKE_KEYS, Set.of(), "右列的 ！ 在侧列，比网格的格子窄"),
    /** 注音九键左列的声调键。 */
    ZHUYIN_TONES(KeyFaceTemplates.ZHUYIN_TONES, Set.of(), null),
    /** 注音九键的音键，底部是发送的数字。 */
    ZHUYIN_SOUNDS(KeyFaceTemplates.ZHUYIN_SOUNDS, Set.of(KeyFaceSlot.BOTTOM), null),
    /** 注音九键最后一行的逗号和句号。 */
    ZHUYIN_MARKS(KeyFaceTemplates.ZHUYIN_MARKS, Set.of(), null),
    /** 日语九键网格的假名键。 */
    JAPANESE_KANA(KeyFaceTemplates.JAPANESE_KANA, Set.of(), null),
    /** 日语九键符号层的数字键。 */
    JAPANESE_DIGITS(KeyFaceTemplates.JAPANESE_KANA, Set.of(), null),
    /** #+= 层第一排的字符键。 */
    LAYER_TOP(KeyFaceTemplates.LAYER_TOP_ROW, Set.of(), null),
    /** #+= 层其余几排（含第五排补充行）的字符键。 */
    LAYER_CHARACTERS(KeyFaceTemplates.LAYER_CHARACTERS, Set.of(), null),
    /** 九键、笔画和数字九宫格左侧的符号栏。 */
    SIDE_SYMBOLS(KeyFaceTemplates.SIDE_SYMBOLS, Set.of(), null),
    /** 符号栏末尾右上角印 0 的 ！（大回车外框）：自己一组，整栏按最宽的符号判断碰不碰角标时不用为它让位。 */
    SIDE_CORNER(KeyFaceTemplates.SIDE_SYMBOLS, Set.of(KeyFaceSlot.TOP_RIGHT), null),

    // 底行（功能行，以及挪进右侧大回车外框、注音九键两列、#+= 层最后一行的同一批常驻键）：键宽各不相同、能画的字也各不相同，每个键各按自己的字定字号，所以每个键一个角色、一组。这些键在建键行之后还留着用，组不随建键行清空（persistent）。
    /** 符键：「符」，九键双拼组字时「分词」（{@link KeyFaceSpecs#SYMBOL_PANEL}）。 */
    BOTTOM_SYMBOL_PANEL(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 123 键和这副键盘的层标题（{@link KeyFaceSpecs#LAYER}）。 */
    BOTTOM_LAYER(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 中/英键（{@link KeyFaceSpecs#LANGUAGE}）。 */
    BOTTOM_LANGUAGE(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 手写底行的「重写」。 */
    BOTTOM_REWRITE(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 底行的 😀。 */
    BOTTOM_EMOJI(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 手写底行的 ⌫。 */
    BOTTOM_DELETE(KeyFaceTemplates.FUNCTIONS, Set.of(), null),
    /** 底行的逗号键，右上角是长按输入的标点。 */
    BOTTOM_COMMA(KeyFaceTemplates.BOTTOM_COMMA, Set.of(KeyFaceSlot.TOP_RIGHT), null),
    /** 底行的句号键，手写底行右上角是长按输入的叹号。 */
    BOTTOM_PERIOD(KeyFaceTemplates.BOTTOM_PERIOD, Set.of(KeyFaceSlot.TOP_RIGHT), null),
    /** 空格：方案短名和麦克风由 {@link SpaceKeyFace} 自己画，字号和图标边长取这一组。 */
    BOTTOM_SPACE(KeyFaceTemplates.SPACE, Set.of(), null),
    /** 回车（{@link KeyFaceSpecs#RETURN}）。 */
    RETURN(KeyFaceTemplates.RETURN, Set.of(KeyFaceSlot.TOP_RIGHT), null),
    /** 右侧大回车外框里的同一个回车：设计尺寸落在两行键高上，字以同一列的功能键为上限。 */
    TALL_RETURN(KeyFaceTemplates.TALL_RETURN, Set.of(KeyFaceSlot.TOP_RIGHT), null);

    private final KeyFaceTemplate template;
    private final Set<KeyFaceSlot> slots;
    private final String mixedWidths;

    KeyFaceRole(KeyFaceTemplate template, Set<KeyFaceSlot> slots, String mixedWidths) {
        this.template = template;
        this.slots = slots;
        this.mixedWidths = mixedWidths;
    }

    public KeyFaceTemplate template() { return template; }

    /** 这个角色的键可能用到的提示位置（主标签总算在内）。 */
    public Set<KeyFaceSlot> slots() { return slots; }

    /** 组里的键有意宽窄不一时写原因；为 {@code null} 时组里的键应当一样大（设备检查核对）。 */
    public String mixedWidths() { return mixedWidths; }

    /** 这副键盘的字母组：功能键的字以它为上限。按这个顺序取第一个这副键盘上有的。 */
    public static final List<KeyFaceRole> KEYBOARD_LETTERS = List.of(LETTERS, FOURTEEN_LETTERS, EIGHTEEN_LETTERS,
        NINE_LETTERS, STROKES, JAPANESE_KANA, JAPANESE_DIGITS);

    /** 主标签以哪一组为上限：按顺序取这副键盘上有的第一个；空表示不另设上限。 */
    public List<KeyFaceRole> leaders() {
        return switch (this) {
            case FUNCTIONS, JAPANESE_FUNCTIONS -> KEYBOARD_LETTERS;
            case LONG_VOWEL -> List.of(LETTERS);
            case STROKE_KEYS -> List.of(STROKES);
            case ZHUYIN_MARKS -> List.of(ZHUYIN_SOUNDS);
            case SIDE_CORNER -> List.of(SIDE_SYMBOLS);
            case BOTTOM_SYMBOL_PANEL, BOTTOM_LAYER, BOTTOM_LANGUAGE, BOTTOM_REWRITE, BOTTOM_EMOJI, BOTTOM_DELETE, RETURN ->
                KEYBOARD_LETTERS;
            case BOTTOM_PERIOD -> List.of(BOTTOM_COMMA);
            case TALL_RETURN -> List.of(FUNCTIONS);
            default -> List.of();
        };
    }

    /** 九键的键用 `sans-serif-medium`（等宽皮肤用等宽字体），与样式通道给九键键帽的字体相同。 */
    public boolean nineKeyTypeface() {
        return this == NINE_LETTERS || this == NINE_DIGITS || this == NINE_SHUANGPIN;
    }

    /** 底行的角色：组不随建键行清空，换了一套控件时才作废（{@code ImeKeyFaces.resetBottomRow}）。 */
    public boolean persistent() {
        return ordinal() >= BOTTOM_SYMBOL_PANEL.ordinal();
    }

    /** 主标签用粗体（回车的「确认」）。 */
    public boolean bold() {
        return this == RETURN || this == TALL_RETURN;
    }

    /** 建键行后还会挪到别处继续用的键（⇧、⌫、底行的 0）放进的角色：建键行时先把它们拿出组。 */
    public boolean movable() {
        return this == FUNCTIONS || this == JAPANESE_FUNCTIONS || this == NINE_DIGITS || this == NINE_SHUANGPIN;
    }
}
