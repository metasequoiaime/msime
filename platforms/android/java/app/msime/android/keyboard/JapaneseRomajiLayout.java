package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/** 日语罗马字键面直接提供长音符；发送 ASCII 减号，由引擎保留在读音中。 */
public final class JapaneseRomajiLayout {
    public static final String LONG_VOWEL_FACE = "ー";
    public static final String LONG_VOWEL_INPUT = "-";

    private JapaneseRomajiLayout() { }

    /** 英文直输和本地模式继续使用原来的字母键面。 */
    public static boolean active(boolean japanese, boolean english, String localMode) {
        return japanese && !english && "none".equals(localMode);
    }

    /** 与 Google 日语 QWERTY 一样，在 L 右侧放长音键，不改变字母顺序或增加键盘高度。 */
    public static List<List<String>> rows(List<List<String>> letters, boolean active) {
        if (!active) return letters;
        List<List<String>> result = new ArrayList<>(letters);
        List<String> second = new ArrayList<>(result.get(1));
        second.add(LONG_VOWEL_INPUT);
        result.set(1, List.copyOf(second));
        return List.copyOf(result);
    }
}
