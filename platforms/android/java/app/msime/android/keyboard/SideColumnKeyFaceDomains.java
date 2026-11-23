package app.msime.android;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 侧栏键组的内容域（见 {@link KeyFaceDomain}）：九键、笔画和数字九宫格左侧符号栏这一次建出来的符号（表可自定义，改了以后整栏重建，所以内容域就是建栏时的表），以及大回车外框接在末尾的 ！ 右上角的 0（那一格自己一组，见 ImeKeyFaces#beginSideSymbols）。只取决于传进来的表，不读服务，JVM 冒烟可以直接检查。
 */
public final class SideColumnKeyFaceDomains {
    private SideColumnKeyFaceDomains() { }

    /** {@code zeroShortcut} 为真时这些符号带着右上角的 0（{@link SuperscriptShortcutPolicy#ZERO}）。 */
    public static KeyFaceDomain symbols(List<String> symbols, boolean zeroShortcut) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, symbols);
        if (zeroShortcut) texts.put(KeyFaceSlot.TOP_RIGHT, List.of(SuperscriptShortcutPolicy.ZERO));
        return new KeyFaceDomain(texts);
    }
}
