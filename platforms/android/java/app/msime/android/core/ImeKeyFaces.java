package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Typeface;
import java.util.Set;

/**
 * 当前这副键盘的键组（{@link KeyFaceGroup}）：建键行时清空，字母键、九键的键和键区功能键放进行里时放进对应的组。只读服务里的设置、把键放进组，排版规则在 {@code KeyFace*} 模型里，内容域在 {@link LetterKeyFaceDomains}、{@link NineKeyFaceDomains} 和 {@link FunctionKeyFaceDomains}。
 *
 * <p>功能键组的字不大过同一副键盘的字母组（{@link KeyFaceGroup#capMainAt}）。⇧、⌫ 和底行的 0 是常驻的键，建键行时会挪到别的地方（123 层、底行），所以清空时先把它们拿出组，放回原处时再放进新组。
 *
 * <p>皮肤字体、键帽形状、字体缩放、角标开关或引号样式变了，{@link #refresh} 在下一次渲染时换掉组的环境和内容域，组里的键随之重画，不重建键行。双拼提示随方案画上时由键自己补进组里（{@link KeyFaceGroup#include}）。
 */
final class ImeKeyFaces {
    private static final Set<KeyFaceSlot> LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM);
    private static final Set<KeyFaceSlot> GRID_LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT);
    private static final Set<KeyFaceSlot> NO_HINTS = Set.of();

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

    /** 功能键的字以同一副键盘的字母为上限：26 键一族是字母组，14/18 键是网格字母组，拼音九键是九键字母组。 */
    private void linkFunctions() {
        if (functions != null)
            functions.capMainAt(letters != null ? letters : gridLetters != null ? gridLetters : nineLetters);
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
