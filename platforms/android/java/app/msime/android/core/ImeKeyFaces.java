package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Typeface;
import java.util.Set;

/**
 * 当前这副键盘的键组（{@link KeyFaceGroup}）：建键行时清空，字母键、九键的键和键区功能键放进行里时放进对应的组。只读服务里的设置、把键放进组，排版规则在 {@code KeyFace*} 模型里，内容域在 {@link LetterKeyFaceDomains}、{@link NineKeyFaceDomains}、{@link FunctionKeyFaceDomains} 和 {@link SideColumnKeyFaceDomains}。
 *
 * <p>功能键组的字不大过同一副键盘的字母组（{@link KeyFaceGroup#capMainAt}）。⇧、⌫ 和底行的 0 是常驻的键，建键行时会挪到别的地方（123 层、底行），所以清空时先把它们拿出组，放回原处时再放进新组。
 *
 * <p>皮肤字体、键帽形状、字体缩放、角标开关或引号样式变了，{@link #refresh} 在下一次渲染时换掉组的环境和内容域，组里的键随之重画，不重建键行。双拼提示随方案画上时由键自己补进组里（{@link KeyFaceGroup#include}）。
 */
final class ImeKeyFaces {
    private static final Set<KeyFaceSlot> LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM);
    private static final Set<KeyFaceSlot> GRID_LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT);
    private static final Set<KeyFaceSlot> NO_HINTS = Set.of();
    /** #+= 层的组一开始的内容域：空的，建这一层时放进来的每个键把自己的文字补进去（{@link KeyFaceGroup#attach}），这一层重建时整组换掉。 */
    private static final KeyFaceDomain LAYER_DOMAIN = new KeyFaceDomain(java.util.Map.of());

    /** 决定内容域、字体和环境的几项；都没变时 {@link #refresh} 什么也不做。 */
    private record Inputs(float density, float fontScale, boolean monospaced, KeyOutline outline, boolean corners,
            ChineseSymbolFaces.Marks marks, boolean variant) { }

    private final MSIMEInputService s;
    private KeyFaceGroup letters;
    private Inputs lettersBuilt;
    private KeyFaceGroup gridLetters;
    private Inputs gridLettersBuilt;
    private KeyFaceGroup functions;
    private Inputs functionsBuilt;
    private KeyFaceGroup numberRow;
    private Inputs numberRowBuilt;
    private KeyFaceGroup nineLetters;
    private Inputs nineLettersBuilt;
    private KeyFaceGroup nineDigits;
    private Inputs nineDigitsBuilt;
    /**
     * 内容域固定、只按设置换环境的一组（韩文、大千、日文罗马字的 ー）：第一次放键进来时建，{@link #refresh} 时只比较环境。
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

        void refresh() {
            if (group == null) return;
            Inputs now = inputs(false);
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

    ImeKeyFaces(MSIMEInputService s) {
        this.s = s;
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
        if (!(key instanceof KeyboardPressButton press)) return;
        if (functions == null) {
            functionsBuilt = inputs(false);
            functions = new KeyFaceGroup(KeyFaceTemplates.FUNCTIONS, environment(functionsBuilt),
                FunctionKeyFaceDomains.functions(), measurer(functionsBuilt));
        }
        press.setFaceGroup(functions, corner ? GRID_LETTER_SLOTS : NO_HINTS);
        if (!movableKeys.contains(press)) movableKeys.add(press);
        linkFunctions();
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
                NineKeyFaceDomains.letters(), nineMeasurer(nineLettersBuilt));
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

    /** 功能键的字以同一副键盘的字母为上限：26 键一族是字母组，14/18 键是网格字母组，拼音九键是九键字母组。 */
    private void linkFunctions() {
        if (functions != null)
            functions.capMainAt(letters != null ? letters : gridLetters != null ? gridLetters : nineLetters);
        if (longVowel.group != null) longVowel.group.capMainAt(letters);
    }

    /** 每次渲染时调用：设置、皮肤字体或字体缩放变了就换掉组的内容域；没变时只比较几项。 */
    void refresh() {
        if (letters != null) {
            Inputs now = inputs(false);
            if (!now.equals(lettersBuilt)) {
                lettersBuilt = now;
                letters.replace(environment(now), measurer(now), letterDomain(now));
            }
        }
        if (gridLetters != null) {
            Inputs now = inputs(gridLettersBuilt.variant());
            if (!now.equals(gridLettersBuilt)) {
                gridLettersBuilt = now;
                gridLetters.replace(environment(now), measurer(now), gridLetterDomain(now));
            }
        }
        if (functions != null) {
            Inputs now = inputs(false);
            if (!now.equals(functionsBuilt)) {
                functionsBuilt = now;
                functions.replace(environment(now), measurer(now), FunctionKeyFaceDomains.functions());
            }
        }
        if (numberRow != null) {
            Inputs now = inputs(false);
            if (!now.equals(numberRowBuilt)) {
                numberRowBuilt = now;
                numberRow.replace(environment(now), measurer(now), LetterKeyFaceDomains.numberRow());
            }
        }
        if (nineLetters != null) {
            Inputs now = inputs(false);
            if (!now.equals(nineLettersBuilt)) {
                nineLettersBuilt = now;
                nineLetters.replace(environment(now), nineMeasurer(now), NineKeyFaceDomains.letters());
            }
        }
        if (nineDigits != null) {
            Inputs now = inputs(nineDigitsBuilt.variant());
            if (!now.equals(nineDigitsBuilt)) {
                nineDigitsBuilt = now;
                nineDigits.replace(environment(now), nineMeasurer(now), nineDigitDomain(now.variant()));
            }
        }
        refreshSideSymbols();
        refreshLayer();
        korean.refresh();
        dachen.refresh();
        longVowel.refresh();
    }

    private void refreshLayer() {
        if (layerBuilt == null) return;
        Inputs now = inputs(false);
        if (now.equals(layerBuilt)) return;
        layerBuilt = now;
        if (layerTop != null) layerTop.replace(environment(now), measurer(now), LAYER_DOMAIN);
        if (layerCharacters != null) layerCharacters.replace(environment(now), measurer(now), LAYER_DOMAIN);
    }

    private void refreshSideSymbols() {
        if (sideSymbols == null) return;
        Inputs now = inputs(false);
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
            s.skin != null && s.skin.monospaced(), KeyOutline.forSkin(s.skin, density),
            s.localSettings.bool(AndroidLocalSettings.LETTER_CORNER_HINTS), s.punctuationMarks(), variant);
    }

    private static KeyFaceEnvironment environment(Inputs inputs) {
        return new KeyFaceEnvironment(inputs.density(), inputs.fontScale(), inputs.outline());
    }

    private static KeyFaceMeasurer measurer(Inputs inputs) {
        return new PaintKeyFaceMeasurer(inputs.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
    }

    private static KeyFaceDomain letterDomain(Inputs inputs) {
        return LetterKeyFaceDomains.letters(inputs.corners(), inputs.marks());
    }

    private static KeyFaceDomain gridLetterDomain(Inputs inputs) {
        return LetterKeyFaceDomains.gridLetters(inputs.variant(), inputs.corners(), inputs.marks());
    }
}
