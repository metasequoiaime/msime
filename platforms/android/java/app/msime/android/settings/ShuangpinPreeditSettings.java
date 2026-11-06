package app.msime.android;

import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Consumer;

/** 键盘页的双拼显示设置行；原生页提供绘制与保存，JVM 冒烟运行同一份行构建和回调。 */
public final class ShuangpinPreeditSettings {
    private ShuangpinPreeditSettings() {}

    @FunctionalInterface
    public interface ToggleRow {
        void add(String title, boolean checked, Consumer<Boolean> change, String help);
    }

    public static void addRows(AppEdition edition, BiFunction<String, Boolean, Boolean> read,
            BiConsumer<String, Boolean> save, ToggleRow rows) {
        boolean raw = read.apply("shuangpin_preedit_uses_raw", true);
        if (edition.offers("shuangpin") || edition.offers("cantonese")) {
            rows.add("双拼显示原始按键", raw,
                checked -> save.accept("shuangpin_preedit_uses_raw", checked),
                "关闭后输入框里显示按键对应的完整拼音或粤拼");
        }
        // 粤语双拼不读分隔字段，只在本版本提供普通话双拼且原始按键开启时列出。
        if (edition.offers("shuangpin") && raw) {
            rows.add("原始按键按音节分隔", read.apply("shuangpin_preedit_separated", false),
                checked -> save.accept("shuangpin_preedit_separated", checked),
                "在原始按键之间按音节加分隔符，例如自然码 nimf 显示为 ni'mf");
        }
    }
}
