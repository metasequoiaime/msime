package app.msime.android;

/** 全拼 26 键组字时 ⇧ 保留还是换成「分词」：共享偏好 `touch_quanpin_shift`，缺省为关（分词），与 client-core 一致。 */
public final class QuanpinSeparatorPolicy {
    /** 打开后全拼组字时 ⇧ 不再被替换，点一下臂辅助码（Shift+字母）。 */
    public static final String PREFERENCE_KEY = "touch_quanpin_shift";

    private QuanpinSeparatorPolicy() { }
}
