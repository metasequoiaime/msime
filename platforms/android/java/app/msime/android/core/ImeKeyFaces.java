package app.msime.android;

import android.content.res.Configuration;
import android.graphics.Typeface;
import java.util.Set;

/**
 * 当前这副键盘的键组（{@link KeyFaceGroup}）：建键行时清空，字母键建出来时放进对应的组。只读服务里的设置、把键放进组，排版规则在 {@code KeyFace*} 模型里，内容域在 {@link LetterKeyFaceDomains}。
 *
 * <p>皮肤字体、字体缩放、角标开关或引号样式变了，{@link #refresh} 在下一次渲染时换掉组的内容域，组里的键随之重画，不重建键行。双拼提示随方案画上时由键自己补进组里（{@link KeyFaceGroup#include}）。
 */
final class ImeKeyFaces {
    private static final Set<KeyFaceSlot> LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT, KeyFaceSlot.BOTTOM);
    private static final Set<KeyFaceSlot> GRID_LETTER_SLOTS = Set.of(KeyFaceSlot.TOP_RIGHT);

    /** 决定内容域、字体和环境的几项；都没变时 {@link #refresh} 什么也不做。 */
    private record Inputs(float density, float fontScale, boolean monospaced, boolean corners,
            ChineseSymbolFaces.Marks marks, boolean eighteen) { }

    private final MSIMEInputService s;
    private KeyFaceGroup letters;
    private Inputs lettersBuilt;
    private KeyFaceGroup gridLetters;
    private Inputs gridLettersBuilt;

    ImeKeyFaces(MSIMEInputService s) {
        this.s = s;
    }

    /** 建键行之前调用：上一副键盘的组不再用，新键放进新组。 */
    void reset() {
        letters = null;
        lettersBuilt = null;
        gridLetters = null;
        gridLettersBuilt = null;
    }

    /** 26 键（和同一套字母行的其他语言）字母层的一个字母键。 */
    void attachLetter(KeyboardPressButton key) {
        if (letters == null) {
            lettersBuilt = inputs(false);
            letters = new KeyFaceGroup(KeyFaceTemplates.LETTERS, environment(lettersBuilt), letterDomain(lettersBuilt),
                measurer(lettersBuilt));
        }
        key.setFaceGroup(letters, LETTER_SLOTS);
    }

    /** 14/18 键的一个字母键。 */
    void attachGridLetter(KeyboardPressButton key, boolean eighteen) {
        if (gridLetters == null || gridLettersBuilt.eighteen() != eighteen) {
            gridLettersBuilt = inputs(eighteen);
            gridLetters = new KeyFaceGroup(KeyFaceTemplates.GRID_LETTERS, environment(gridLettersBuilt),
                gridLetterDomain(gridLettersBuilt), measurer(gridLettersBuilt));
        }
        key.setFaceGroup(gridLetters, GRID_LETTER_SLOTS);
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
