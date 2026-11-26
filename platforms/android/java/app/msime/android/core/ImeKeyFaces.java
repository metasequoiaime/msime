package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Typeface;
import app.msime.android.core.InputViewValuePolicy;
import java.util.Set;

/**
 * 当前这副键盘的键组（{@link KeyFaceGroup}）：建键行时清空，字母键、九键的键和键区功能键放进行里时放进对应的组。只读服务里的设置、把键放进组，排版规则在 {@code KeyFace*} 模型里，内容域在 {@link LetterKeyFaceDomains}、{@link NineKeyFaceDomains}（含笔画键盘、注音九键和日语九键）、{@link KeyFaceSpecs} 和 {@link SideColumnKeyFaceDomains}，都按这副键盘的 {@link KeyFaceLayout} 列出。键面会变的键（⇧、14/18 键的分词键、九键的 1 键、日语九键的几个侧列键）由 {@link #show} 按它们的 {@link KeyFaceSpec} 设上文字或图标。
 *
 * <p>功能键组的字不大过同一副键盘的字母组（{@link KeyFaceGroup#capMainAt}）。⇧、⌫ 和底行的 0 是常驻的键，建键行时会挪到别的地方（123 层、底行），所以清空时先把它们拿出组，放回原处时再放进新组。
 *
 * <p>皮肤字体、键帽形状、字体缩放或 {@link KeyFaceLayout} 里的设置（角标开关、引号样式、双拼提示表……）变了，{@link #refresh} 在下一次渲染时换掉组的环境和内容域，组里的键随之重画，不重建键行。键画上列表里没有的字是列表漏了（{@link KeyFaceGroup#include}），记一行日志。
 */
final class ImeKeyFaces {
    private static final Set<KeyFaceSlot> LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM);
    private static final Set<KeyFaceSlot> GRID_LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT);
    private static final Set<KeyFaceSlot> NO_HINTS = Set.of();
    /** #+= 层的组一开始的内容域：空的，建这一层时放进来的每个键把自己的文字补进去（{@link KeyFaceGroup#attach}），这一层重建时整组换掉。 */
    private static final KeyFaceDomain LAYER_DOMAIN = new KeyFaceDomain(java.util.Map.of());

    /** 决定内容域、字体和环境的几项；都没变时 {@link #refresh} 什么也不做。 */
    private record Inputs(float density, float fontScale, boolean monospaced, KeyOutline outline, KeyFaceLayout layout,
            boolean variant) {
        Inputs withVariant(boolean value) {
            return value == variant ? this : new Inputs(density, fontScale, monospaced, outline, layout, value);
        }
    }

    private final MSIMEInputService s;
    private KeyFaceGroup letters;
    private Inputs lettersBuilt;
    private KeyFaceGroup gridLetters;
    private Inputs gridLettersBuilt;
    private KeyFaceGroup functions;
    private Inputs functionsBuilt;
    /** 这副键盘是日语九键：功能键组用它自己的模板（{@link KeyFaceTemplates#JAPANESE_FUNCTIONS}），内容域另含日语九键侧列和左下角的文字键（{@link KeyFaceSpecs#functionTexts}）。 */
    private boolean japaneseFunctions;
    private KeyFaceGroup numberRow;
    private Inputs numberRowBuilt;
    private KeyFaceGroup nineLetters;
    private Inputs nineLettersBuilt;
    private KeyFaceGroup nineDigits;
    private Inputs nineDigitsBuilt;
    /**
     * 内容域固定、只按设置换环境的一组（韩文、大千、日文罗马字的 ー、笔画键盘、注音九键、日语九键）：第一次放键进来时建，{@link #refresh} 时只比较环境。
     */
    private final class FixedGroup {
        private final KeyFaceTemplate template;
        private final KeyFaceDomain domain;
        private KeyFaceGroup group;
        private Inputs built;

        FixedGroup(KeyFaceTemplate template, KeyFaceDomain domain) {
            this.template = template;
            this.domain = domain;
        }

        KeyFaceGroup group() {
            if (group == null) {
                built = inputs(false);
                group = new KeyFaceGroup(template, environment(built), domain, measurer(built));
            }
            return group;
        }

        void refresh(Inputs now) {
            if (group == null) return;
            if (now.equals(built)) return;
            built = now;
            group.replace(environment(now), measurer(now), domain);
        }

        void reset() {
            group = null;
            built = null;
        }
    }

    private final FixedGroup korean = new FixedGroup(KeyFaceTemplates.KOREAN, LetterKeyFaceDomains.korean());
    private final FixedGroup dachen = new FixedGroup(KeyFaceTemplates.DACHEN, LetterKeyFaceDomains.dachen());
    /** 日文罗马字的长音键 ー：自己一组、以字母组为上限，全角的 ー 放进字母组会让窄键上的每个字母都按它缩小。 */
    private final FixedGroup longVowel = new FixedGroup(KeyFaceTemplates.LETTERS, LAYER_DOMAIN);
    /** 笔画键盘的笔画键。 */
    private final FixedGroup strokes = new FixedGroup(KeyFaceTemplates.STROKES, NineKeyFaceDomains.strokes());
    /** 笔画键盘的 @#、0、句点和 ！：主标签以笔画组为上限。 */
    private final FixedGroup strokeKeys = new FixedGroup(KeyFaceTemplates.STROKE_KEYS, NineKeyFaceDomains.strokeKeys());
    /** 注音九键左列的声调键。 */
    private final FixedGroup zhuyinTones = new FixedGroup(KeyFaceTemplates.ZHUYIN_TONES, NineKeyFaceDomains.zhuyinTones());
    /** 注音九键的音键：注音符号和底部的数字。 */
    private final FixedGroup zhuyinSounds = new FixedGroup(KeyFaceTemplates.ZHUYIN_SOUNDS, NineKeyFaceDomains.zhuyinSounds());
    /** 注音九键最后一行的逗号和句号：主标签以音键组为上限。 */
    private final FixedGroup zhuyinMarks = new FixedGroup(KeyFaceTemplates.ZHUYIN_MARKS, NineKeyFaceDomains.zhuyinMarks());
    /** 日语九键网格的假名键，和符号层上的数字键（两层各一组）。 */
    private final FixedGroup japaneseKana = new FixedGroup(KeyFaceTemplates.JAPANESE_KANA, NineKeyFaceDomains.japaneseKana(false));
    private final FixedGroup japaneseDigits = new FixedGroup(KeyFaceTemplates.JAPANESE_KANA, NineKeyFaceDomains.japaneseKana(true));

    /** #+= 层的字符键：第一排一组，其余几排（含第五排补充行）一组。内容域由放进来的键自己的文字组成（{@link #LAYER_DOMAIN}）。 */
    private KeyFaceGroup layerTop;
    private KeyFaceGroup layerCharacters;
    private Inputs layerBuilt;
    private KeyFaceGroup sideSymbols;
    private Inputs sideSymbolsBuilt;
    /** 符号栏的内容域：建栏时的表。 */
    private KeyFaceDomain sideSymbolsDomain;
    /** 符号栏里带 0 的 ！ 自己一组（{@link #beginSideSymbols}）；这一栏没有时为 {@code null}。 */
    private KeyFaceGroup sideCorner;
    private KeyFaceDomain sideCornerDomain;
    /** 放进过组的键里建键行后还会留着用的那些（⇧、⌫、底行的 0 等），清空时拿出组。 */
    private final java.util.List<KeyboardPressButton> movableKeys = new java.util.ArrayList<>();

    /** 已经记过日志的漏网文字（键组模板名、位置、文字），每种只记一次。 */
    private final Set<String> strays = new java.util.HashSet<>();

    ImeKeyFaces(MSIMEInputService s) {
        this.s = s;
        KeyFaceGroup.setStrayListener((group, slot, text) -> {
            if (strays.add(group.template().name() + " " + slot + " " + text))
                ImeLog.w("key face list misses " + group.template().name() + " " + slot + " 「" + text + "」");
        });
    }

    /** 建键行之前调用：上一副键盘的组不再用，新键放进新组。 */
    void reset() {
        letters = null;
        lettersBuilt = null;
        gridLetters = null;
        gridLettersBuilt = null;
        for (KeyboardPressButton key : movableKeys) if (key.faceGroup() != null) key.setFaceGroup(null, NO_HINTS);
        movableKeys.clear();
        functions = null;
        functionsBuilt = null;
        japaneseFunctions = false;
        numberRow = null;
        numberRowBuilt = null;
        nineLetters = null;
        nineLettersBuilt = null;
        nineDigits = null;
        nineDigitsBuilt = null;
        layerTop = null;
        layerCharacters = null;
        layerBuilt = null;
        korean.reset();
        dachen.reset();
        longVowel.reset();
        strokes.reset();
        strokeKeys.reset();
        zhuyinTones.reset();
        zhuyinSounds.reset();
        zhuyinMarks.reset();
        japaneseKana.reset();
        japaneseDigits.reset();
        sideSymbols = null;
        sideSymbolsBuilt = null;
        sideSymbolsDomain = null;
        sideCorner = null;
        sideCornerDomain = null;
    }

    /** 26 键（和同一套字母行的其他语言）字母层的一个字母键。 */
    void attachLetter(KeyboardPressButton key) {
        if (letters == null) {
            lettersBuilt = inputs(false);
            letters = new KeyFaceGroup(KeyFaceTemplates.LETTERS, environment(lettersBuilt), letterDomain(lettersBuilt),
                measurer(lettersBuilt));
        }
        key.setFaceGroup(letters, LETTER_SLOTS);
        linkFunctions();
    }

    /** 14/18 键的一个字母键。 */
    void attachGridLetter(KeyboardPressButton key, boolean eighteen) {
        if (gridLetters == null || gridLettersBuilt.variant() != eighteen) {
            gridLettersBuilt = inputs(eighteen);
            gridLetters = new KeyFaceGroup(KeyFaceTemplates.GRID_LETTERS, environment(gridLettersBuilt),
                gridLetterDomain(gridLettersBuilt), measurer(gridLettersBuilt));
        }
        key.setFaceGroup(gridLetters, GRID_LETTER_SLOTS);
        linkFunctions();
    }

    /** 键区里的一个功能键（⇧、⌫、分词键、九键右列的重输）；{@code corner} 为真时它可能画右上角提示（14/18 键分词键的「辅」）。不是 {@link KeyboardPressButton} 的键照旧自己画。 */
    void attachFunction(android.widget.Button key, boolean corner) {
        attachFunction(key, corner, null);
    }

    /** 键面会变的功能键：按 {@code spec} 放进组（它的全部状态已经在组的内容域里），随即按此刻的状态设上键面，常驻的键（⇧）不会带着上一副键盘的字进来。 */
    void attachFunction(android.widget.Button key, KeyFaceSpec<?> spec) {
        attachFunction(key, spec.drawsCorner(), spec);
    }

    private void attachFunction(android.widget.Button key, boolean corner, KeyFaceSpec<?> spec) {
        if (!(key instanceof KeyboardPressButton press)) return;
        if (functions == null) {
            functionsBuilt = inputs(false);
            functions = new KeyFaceGroup(japaneseFunctions ? KeyFaceTemplates.JAPANESE_FUNCTIONS : KeyFaceTemplates.FUNCTIONS,
                environment(functionsBuilt), functionDomain(functionsBuilt), measurer(functionsBuilt));
        }
        press.setFaceGroup(functions, corner ? GRID_LETTER_SLOTS : NO_HINTS, spec == null);
        if (spec != null) show(key, spec);
        if (!movableKeys.contains(press)) movableKeys.add(press);
        linkFunctions();
    }

    /**
     * 按 {@code spec} 和此刻的状态设上键面：画文字的键换上这段字（节点文字随之换），画图标的键（{@link KeyboardIconKey}）换图标；{@link KeyFaceSpec#drawsCorner} 时连右上角一起设。渲染时调用，代替各处自己写字。
     */
    void show(android.widget.Button key, KeyFaceSpec<?> spec) {
        if (key == null) return;
        KeyFace face = spec.face(layout(), live());
        if (key instanceof KeyboardIconKey icon) {
            icon.setFace(face);
            return;
        }
        if (face.text() != null && !face.text().contentEquals(key.getText())) key.setText(face.text());
        if (spec.drawsCorner() && key instanceof KeyHintButton hint) hint.setCornerHint(face.corner());
    }

    /** 建键时的文字：{@code spec} 此刻的状态写的字（画图标时为空串）。 */
    String text(KeyFaceSpec<?> spec) {
        return spec.face(layout(), live()).mainText();
    }

    /** 建日语九键之前调用（在放进第一个功能键之前）：功能键组另含日语九键两侧列的文字键和「小゛゜」，字号和图标按侧列一格的高度定。 */
    void useJapaneseFunctions() {
        japaneseFunctions = true;
    }

    private static KeyFaceDomain functionDomain(Inputs inputs) {
        return KeyFaceSpecs.functionDomain(inputs.layout());
    }

    /** 字母上方数字行（26 键一族、14/18 键）的一个数字键。 */
    void attachNumber(android.widget.Button key) {
        if (!(key instanceof KeyboardPressButton press)) return;
        if (numberRow == null) {
            numberRowBuilt = inputs(false);
            numberRow = new KeyFaceGroup(KeyFaceTemplates.NUMBER_ROW, environment(numberRowBuilt),
                LetterKeyFaceDomains.numberRow(), measurer(numberRowBuilt));
        }
        press.setFaceGroup(numberRow, NO_HINTS);
    }

    /** 拼音九键字母层的一个键（九格和右列的 ！）：主标签是字母组，右上角是长按输入的数字。 */
    void attachNineLetter(android.widget.Button key) {
        if (!(key instanceof KeyboardPressButton press)) return;
        if (nineLetters == null) {
            nineLettersBuilt = inputs(false);
            nineLetters = new KeyFaceGroup(KeyFaceTemplates.NINE_LETTERS, environment(nineLettersBuilt),
                NineKeyFaceDomains.letters(nineLettersBuilt.layout()), nineMeasurer(nineLettersBuilt));
        }
        press.setFaceGroup(nineLetters, GRID_LETTER_SLOTS);
        linkFunctions();
    }

    /**
     * 数字九宫格或九键双拼的一个数字键（含右列的小数点、！ 和底行的 0）；{@code shuangpin} 为真时底部画声韵提示。已经在这个组里时什么也不做：底行的 0 每次渲染都会经过这里。
     */
    void attachNineDigit(android.widget.Button key, boolean shuangpin) {
        if (!(key instanceof KeyboardPressButton press)) return;
        if (nineDigits == null || nineDigitsBuilt.variant() != shuangpin) {
            nineDigitsBuilt = inputs(shuangpin);
            nineDigits = new KeyFaceGroup(KeyFaceTemplates.NINE_DIGITS, environment(nineDigitsBuilt),
                nineDigitDomain(shuangpin), nineMeasurer(nineDigitsBuilt));
        }
        if (press.faceGroup() == nineDigits) return;
        press.setFaceGroup(nineDigits, shuangpin ? Set.of(KeyFaceSlot.BOTTOM) : NO_HINTS);
        if (!movableKeys.contains(press)) movableKeys.add(press);
    }

    /** 韩文字母层的一个字母键。 */
    void attachKorean(android.widget.Button key) {
        if (key instanceof KeyboardPressButton press) press.setFaceGroup(korean.group(), NO_HINTS);
    }

    /** 大千字母层的一个注音或声调键。 */
    void attachDachen(android.widget.Button key) {
        if (key instanceof KeyboardPressButton press) press.setFaceGroup(dachen.group(), NO_HINTS);
    }

    /** 日文罗马字的长音键 ー：主标签以同一副键盘的字母为上限，所以不比字母大。 */
    void attachLongVowel(android.widget.Button key) {
        if (!(key instanceof KeyboardPressButton press)) return;
        press.setFaceGroup(longVowel.group(), NO_HINTS);
        linkFunctions();
    }

    /** 笔画键盘的一个键：{@code stroke} 为真时是笔画键，否则是 @#、0、句点或右列的 ！。 */
    void attachStroke(android.widget.Button key, boolean stroke) {
        if (!(key instanceof KeyboardPressButton press)) return;
        press.setFaceGroup(stroke ? strokes.group() : strokeKeys.group(), NO_HINTS);
        linkFunctions();
    }

    /** 注音九键左列的一个声调键。 */
    void attachZhuyinTone(android.widget.Button key) {
        if (key instanceof KeyboardPressButton press) press.setFaceGroup(zhuyinTones.group(), NO_HINTS);
    }

    /** 注音九键的一个音键（1–9 和 0）：底部画它发送的数字（{@link BottomLabelKey}）。 */
    void attachZhuyinSound(android.widget.Button key) {
        if (!(key instanceof KeyboardPressButton press)) return;
        press.setFaceGroup(zhuyinSounds.group(), Set.of(KeyFaceSlot.BOTTOM));
        linkFunctions();
    }

    /** 注音九键最后一行的逗号或句号。 */
    void attachZhuyinMark(android.widget.Button key) {
        if (!(key instanceof KeyboardPressButton press)) return;
        press.setFaceGroup(zhuyinMarks.group(), NO_HINTS);
        linkFunctions();
    }

    /** 日语九键网格的一个假名键；{@code symbols} 为真时是符号层上的数字键。 */
    void attachJapaneseKana(android.widget.Button key, boolean symbols) {
        if (!(key instanceof KeyboardPressButton press)) return;
        press.setFaceGroup(symbols ? japaneseDigits.group() : japaneseKana.group(), NO_HINTS);
        linkFunctions();
    }

    /** #+= 层的一个字符键；{@code firstRow} 为真时是第一排（字号大一号）。 */
    void attachLayerCharacter(android.widget.Button key, boolean firstRow) {
        if (!(key instanceof KeyboardPressButton press)) return;
        if (layerBuilt == null) layerBuilt = inputs(false);
        KeyFaceGroup group;
        if (firstRow) {
            if (layerTop == null) layerTop = new KeyFaceGroup(KeyFaceTemplates.LAYER_TOP_ROW, environment(layerBuilt),
                LAYER_DOMAIN, measurer(layerBuilt));
            group = layerTop;
        } else {
            if (layerCharacters == null) layerCharacters = new KeyFaceGroup(KeyFaceTemplates.LAYER_CHARACTERS,
                environment(layerBuilt), LAYER_DOMAIN, measurer(layerBuilt));
            group = layerCharacters;
        }
        press.setFaceGroup(group, NO_HINTS);
    }

    /**
     * 九键、笔画或数字九宫格左侧符号栏这一次建出来的符号：{@code symbols} 是整栏的表，{@code zeroSymbol} 是右上角印 0 的那一格（大回车外框接在末尾的 ！），没有时为 {@code null}。每次建栏先调这里，再把每一格 {@link #attachSideSymbol} 进来；一副键盘只有一条符号栏。
     *
     * <p>带 0 的那一格自己一组，主标签以整栏为上限（{@link KeyFaceGroup#capMainAt}），所以和整栏一样大、不会更大：整栏按最宽的符号（……）的框判断碰不碰右上角，放在一组里时这个框会碰上只有 ！ 才有的 0，整栏都要为它左移或缩小。
     */
    void beginSideSymbols(java.util.List<String> symbols, String zeroSymbol) {
        sideSymbolsBuilt = inputs(false);
        sideSymbolsDomain = SideColumnKeyFaceDomains.symbols(symbols, false);
        sideSymbols = new KeyFaceGroup(KeyFaceTemplates.SIDE_SYMBOLS, environment(sideSymbolsBuilt), sideSymbolsDomain,
            measurer(sideSymbolsBuilt));
        sideCorner = null;
        sideCornerDomain = null;
        if (zeroSymbol == null) return;
        sideCornerDomain = SideColumnKeyFaceDomains.symbols(java.util.List.of(zeroSymbol), true);
        sideCorner = new KeyFaceGroup(KeyFaceTemplates.SIDE_SYMBOLS, environment(sideSymbolsBuilt), sideCornerDomain,
            measurer(sideSymbolsBuilt));
        sideCorner.capMainAt(sideSymbols);
    }

    /** 符号栏的一格；{@code corner} 为真时它是右上角印 0 的那一格。 */
    void attachSideSymbol(android.widget.Button key, boolean corner) {
        KeyFaceGroup group = corner ? sideCorner : sideSymbols;
        if (!(key instanceof KeyboardPressButton press) || group == null) return;
        press.setFaceGroup(group, corner ? GRID_LETTER_SLOTS : NO_HINTS);
    }

    /** 功能键的字以同一副键盘的字母为上限：26 键一族是字母组，14/18 键是网格字母组，拼音九键是九键字母组，笔画键盘是笔画组，日语九键是假名组（符号层是数字组）。 */
    private void linkFunctions() {
        if (functions != null) functions.capMainAt(letters != null ? letters : gridLetters != null ? gridLetters
            : nineLetters != null ? nineLetters : strokes.group != null ? strokes.group
            : japaneseKana.group != null ? japaneseKana.group : japaneseDigits.group);
        if (longVowel.group != null) longVowel.group.capMainAt(letters);
        if (strokeKeys.group != null) strokeKeys.group.capMainAt(strokes.group);
        if (zhuyinMarks.group != null) zhuyinMarks.group.capMainAt(zhuyinSounds.group);
    }

    /** 每次渲染时调用：设置、皮肤字体或字体缩放变了就换掉组的内容域；没变时只比较几项。 */
    void refresh() {
        // 每次渲染都会走到这里（一次渲染几回）：KeyFaceLayout 只读一次，各组拿它比较。
        Inputs base = inputs(false);
        if (letters != null) {
            Inputs now = base;
            if (!now.equals(lettersBuilt)) {
                lettersBuilt = now;
                letters.replace(environment(now), measurer(now), letterDomain(now));
            }
        }
        if (gridLetters != null) {
            Inputs now = base.withVariant(gridLettersBuilt.variant());
            if (!now.equals(gridLettersBuilt)) {
                gridLettersBuilt = now;
                gridLetters.replace(environment(now), measurer(now), gridLetterDomain(now));
            }
        }
        if (functions != null) {
            Inputs now = base;
            if (!now.equals(functionsBuilt)) {
                functionsBuilt = now;
                functions.replace(environment(now), measurer(now), functionDomain(now));
            }
        }
        if (numberRow != null) {
            Inputs now = base;
            if (!now.equals(numberRowBuilt)) {
                numberRowBuilt = now;
                numberRow.replace(environment(now), measurer(now), LetterKeyFaceDomains.numberRow());
            }
        }
        if (nineLetters != null) {
            Inputs now = base;
            if (!now.equals(nineLettersBuilt)) {
                nineLettersBuilt = now;
                nineLetters.replace(environment(now), nineMeasurer(now), NineKeyFaceDomains.letters(now.layout()));
            }
        }
        if (nineDigits != null) {
            Inputs now = base.withVariant(nineDigitsBuilt.variant());
            if (!now.equals(nineDigitsBuilt)) {
                nineDigitsBuilt = now;
                nineDigits.replace(environment(now), nineMeasurer(now), nineDigitDomain(now.variant()));
            }
        }
        refreshSideSymbols(base);
        refreshLayer(base);
        korean.refresh(base);
        dachen.refresh(base);
        longVowel.refresh(base);
        strokes.refresh(base);
        strokeKeys.refresh(base);
        zhuyinTones.refresh(base);
        zhuyinSounds.refresh(base);
        zhuyinMarks.refresh(base);
        japaneseKana.refresh(base);
        japaneseDigits.refresh(base);
    }

    private void refreshLayer(Inputs now) {
        if (layerBuilt == null) return;
        if (now.equals(layerBuilt)) return;
        layerBuilt = now;
        if (layerTop != null) layerTop.replace(environment(now), measurer(now), LAYER_DOMAIN);
        if (layerCharacters != null) layerCharacters.replace(environment(now), measurer(now), LAYER_DOMAIN);
    }

    private void refreshSideSymbols(Inputs now) {
        if (sideSymbols == null) return;
        if (now.equals(sideSymbolsBuilt)) return;
        sideSymbolsBuilt = now;
        sideSymbols.replace(environment(now), measurer(now), sideSymbolsDomain);
        if (sideCorner != null) sideCorner.replace(environment(now), measurer(now), sideCornerDomain);
    }

    /** 九键的键用 `sans-serif-medium`，等宽皮肤用等宽字体，与样式通道给九键键帽的字体相同（ImeStyler）。 */
    private static KeyFaceMeasurer nineMeasurer(Inputs inputs) {
        return new PaintKeyFaceMeasurer(inputs.monospaced() ? Typeface.MONOSPACE
            : Typeface.create("sans-serif-medium", Typeface.NORMAL));
    }

    private KeyFaceDomain nineDigitDomain(boolean shuangpin) {
        return shuangpin ? NineKeyFaceDomains.shuangpin(s.nineKeyShuangpinFaces()) : NineKeyFaceDomains.digits();
    }

    /** {@code variant} 区分同一类组的两种内容域：14/18 键里是 18 键，九键数字组里是九键双拼。 */
    private Inputs inputs(boolean variant) {
        Configuration configuration = s.getResources().getConfiguration();
        float density = s.getResources().getDisplayMetrics().density;
        return new Inputs(density, KeyboardTypographyPolicy.keyboardFontScale(configuration.fontScale),
            s.skin != null && s.skin.monospaced(), KeyOutline.forSkin(s.skin, density), layout(), variant);
    }

    /** 这副键盘的 {@link KeyFaceLayout}：显示的触屏布局和决定键面能画哪些字的设置。 */
    KeyFaceLayout layout() {
        java.util.List<String> hints = new java.util.ArrayList<>();
        if (s.shuangpinKeyHintsEnabled && s.shuangpinHints != null)
            for (String hint : s.shuangpinHints.values()) if (hint != null && !hint.isEmpty() && !hints.contains(hint)) hints.add(hint);
        return new KeyFaceLayout(s.displayedTouchLayout(s.view), s.quanpinShiftEnabled, s.nineKeyCompositionAction,
            s.localSettings.bool(AndroidLocalSettings.LETTER_CORNER_HINTS), s.punctuationMarks(),
            s.localSettings.bool(AndroidLocalSettings.LANGUAGE_KEY_CYCLE), s.visibleSchemes, hints);
    }

    /** 此刻随打字、切层变化的几项（{@link KeyFaceLive}）。 */
    KeyFaceLive live() {
        String localMode = s.view == null ? "none" : InputViewValuePolicy.textOr(s.view, "local_mode", "none");
        return new KeyFaceLive(s.hasEngineComposition(), s.keyboardLayer == KeyboardLayout.Layer.SYMBOLS, s.dedicatedEnglish,
            s.session != 0 && s.view != null && !s.dedicatedEnglish && !EditorPolicy.password(s.editorInputType),
            "none".equals(localMode), s.letterCase.mode() == EnglishLetterCaseState.Mode.CAPS_LOCK,
            ImeGridHelpcodeEntry.available(s), s.view == null ? -1 : InputViewValuePolicy.scheme(s.view, -1), s.selectedScheme);
    }

    private static KeyFaceEnvironment environment(Inputs inputs) {
        return new KeyFaceEnvironment(inputs.density(), inputs.fontScale(), inputs.outline());
    }

    private static KeyFaceMeasurer measurer(Inputs inputs) {
        return new PaintKeyFaceMeasurer(inputs.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
    }

    private static KeyFaceDomain letterDomain(Inputs inputs) {
        return LetterKeyFaceDomains.letters(inputs.layout());
    }

    private static KeyFaceDomain gridLetterDomain(Inputs inputs) {
        return LetterKeyFaceDomains.gridLetters(inputs.variant(), inputs.layout());
    }
}
