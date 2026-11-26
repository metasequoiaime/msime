package app.msime.android;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * 一副键盘显示期间不变的、决定键面能画哪些字的几项：显示的触屏布局和几项设置。内容域按它列出（{@link KeyFaceSpec#states}），它变了 {@code ImeKeyFaces.refresh} 换掉内容域；打字、切层时会变的在 {@link KeyFaceLive} 里，不在这里，所以打字时内容域不变、键面不重量。字母层和 123 层不在这里（层是键的状态）：常驻的键切层前后同一个字号。
 *
 * <p>由 {@code ImeKeyFaces} 从服务读出来，这里不读服务，所以 JVM 冒烟可以把每种布局都列一遍。
 *
 * @param touchLayout 显示的触屏布局（{@link KeyboardLayout} 的常量）
 * @param quanpinShift 「全拼 Shift 键切大小写」开着：26 键全拼的 ⇧ 不换成「分词」
 * @param nineKeyAction 九键组字时 1 键的动作（{@link NineKeyCompositionActionPolicy}）
 * @param corners 「显示字母键角标」开着
 * @param marks 引号样式
 * @param languageCycles 「中英键轮换其他语言」开着
 * @param schemes 可选的方案（{@code visibleSchemes}）：轮换时语言键画其中其他语言的字（{@link KeyboardScheme#glyph}）
 * @param shuangpinHints 当前双拼方案每个字母键的提示（空表示没有）
 */
public record KeyFaceLayout(int touchLayout, boolean quanpinShift, String nineKeyAction, boolean corners,
        ChineseSymbolFaces.Marks marks, boolean languageCycles, List<KeyboardScheme> schemes,
        List<String> shuangpinHints) {
    public KeyFaceLayout {
        nineKeyAction = NineKeyCompositionActionPolicy.normalize(nineKeyAction);
        schemes = Collections.unmodifiableList(new ArrayList<>(schemes == null ? List.of() : schemes));
        shuangpinHints = Collections.unmodifiableList(new ArrayList<>(shuangpinHints == null ? List.of() : shuangpinHints));
    }

    /** 这是不是日语九键。 */
    public boolean japaneseNineKey() {
        return touchLayout == KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT;
    }
}
