package app.msime.android;

import java.util.List;

/** 符号栏键组的内容域：就是建栏时的表；只有带 0 的 ！ 那一栏才有右上角。 */
public final class SideColumnKeyFaceDomainsSmoke {
    private SideColumnKeyFaceDomainsSmoke() { }

    public static void main(String[] args) {
        List<String> symbols = List.of("，", "。", "？", "！", "……", "、");
        KeyFaceDomain plain = SideColumnKeyFaceDomains.symbols(symbols, false);
        check(plain.texts().get(KeyFaceSlot.MAIN).equals(symbols), "符号栏的内容域应是建栏时的表：" + plain);
        check(!plain.has(KeyFaceSlot.TOP_RIGHT), "没有带 0 的 ！ 时不应留右上角");
        KeyFaceDomain zero = SideColumnKeyFaceDomains.symbols(symbols, true);
        check(zero.texts().get(KeyFaceSlot.TOP_RIGHT).equals(List.of(SuperscriptShortcutPolicy.ZERO)),
            "大回车外框的 ！ 右上角应是 0：" + zero);
        check(SideColumnKeyFaceDomains.symbols(List.of(), false).texts().isEmpty(), "空表没有内容");
        System.out.println("SideColumnKeyFaceDomainsSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
