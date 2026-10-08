package app.msime.android;

import java.util.Map;

/** 为宿主模型和视图统一规范化不可变映射。 */
public final class MapPolicy {
    private MapPolicy() {}

    /** 把缺失映射当作空映射，并返回不可变副本。 */
    public static <K, V> Map<K, V> copyOrEmpty(Map<? extends K, ? extends V> values) {
        return values == null ? Map.of() : Map.copyOf(values);
    }
}
