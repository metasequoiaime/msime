package app.msime.android;

import java.util.Set;

/** 为宿主模型和视图统一规范化不可变集合。 */
public final class SetPolicy {
    private SetPolicy() {}

    /** 把缺失集合当作空集合，并返回不可变副本。 */
    public static <T> Set<T> copyOrEmpty(Set<? extends T> values) {
        return values == null ? Set.of() : Set.copyOf(values);
    }
}
