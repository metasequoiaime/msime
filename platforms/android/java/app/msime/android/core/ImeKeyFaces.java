package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Typeface;
import java.util.Set;

/**
 * 当前这副键盘的键组（{@link KeyFaceGroup}）：建键行时清空，字母键和键区功能键放进行里时放进对应的组。只读服务里的设置、把键放进组，排版规则在 {@code KeyFace*} 模型里，内容域在 {@link LetterKeyFaceDomains} 和 {@link FunctionKeyFaceDomains}。
 *
 * <p>功能键组的字不大过同一副键盘的字母组（{@link KeyFaceGroup#capMainAt}）。⇧、⌫ 是常驻的键，建键行时会挪到别的地方（123 层、底行），所以清空时先把功能键都拿出组，放回键区时再放进新组。
 *
 * <p>皮肤字体、字体缩放、角标开关或引号样式变了，{@link #refresh} 在下一次渲染时换掉组的内容域，组里的键随之重画，不重建键行。双拼提示随方案画上时由键自己补进组里（{@link KeyFaceGroup#include}）。
 */
final class ImeKeyFaces {
    private static final Set<KeyFaceSlot> LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM);
    private static final Set<KeyFaceSlot> GRID_LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT);
    private static final Set<KeyFaceSlot> NO_HINTS = Set.of();

    /** 决定内容域、字体和环境的几项；都没变时 {@link #refresh} 什么也不做。 */
    private record Inputs(float density, float fontScale, boolean monospaced, boolean corners,
            ChineseSymbolFaces.Marks marks, boolean eighteen) { }

    private final MSIMEInputService s;
    private KeyFaceGroup letters;
    private Inputs lettersBuilt;
    private KeyFaceGroup gridLetters;
    private Inputs gridLettersBuilt;
    private KeyFaceGroup functions;
    private Inputs functionsBuilt;
    private final java.util.List<KeyboardPressButton> functionKeys = new java.util.ArrayList<>();

    ImeKeyFaces(MSIMEInputService s) {
        this.s = s;
    }

    /** 建键行之前调用：上一副键盘的组不再用，新键放进新组。 */
    void reset() {
        letters = null;
        lettersBuilt = null;
        gridLetters = null;
        gridLettersBuilt = null;
        for (KeyboardPressButton key : functionKeys) if (key.faceGroup() == functions) key.setFaceGroup(null, NO_HINTS);
        functionKeys.clear();
        functions = null;
        functionsBuilt = null;
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
        if (gridLetters == null || gridLettersBuilt.eighteen() != eighteen) {
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
        if (!functionKeys.contains(press)) functionKeys.add(press);
        linkFunctions();
    }

    /** 功能键的字以同一副键盘的字母为上限：26 键一族是字母组，14/18 键是网格字母组；九键的键还没放进键组，不设上限。 */
    private void linkFunctions() {
        if (functions != null) functions.capMainAt(letters != null ? letters : gridLetters);
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
            Inputs now = inputs(gridLettersBuilt.eighteen());
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
    }

    private Inputs inputs(boolean eighteen) {
        Configuration configuration = s.getResources().getConfiguration();
        return new Inputs(s.getResources().getDisplayMetrics().density,
            KeyboardTypographyPolicy.keyboardFontScale(configuration.fontScale),
            s.skin != null && s.skin.monospaced(),
            s.localSettings.bool(AndroidLocalSettings.LETTER_CORNER_HINTS), s.punctuationMarks(), eighteen);
    }

    private static KeyFaceEnvironment environment(Inputs inputs) {
        return new KeyFaceEnvironment(inputs.density(), inputs.fontScale());
    }

    private static KeyFaceMeasurer measurer(Inputs inputs) {
        return new PaintKeyFaceMeasurer(inputs.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
    }

    private static KeyFaceDomain letterDomain(Inputs inputs) {
        return LetterKeyFaceDomains.letters(inputs.corners(), inputs.marks());
    }

    private static KeyFaceDomain gridLetterDomain(Inputs inputs) {
        return LetterKeyFaceDomains.gridLetters(inputs.eighteen(), inputs.corners(), inputs.marks());
    }
}
