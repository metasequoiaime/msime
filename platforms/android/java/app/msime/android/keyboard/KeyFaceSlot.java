package app.msime.android;

/** 键面上的位置：中央的主标签和几处提示。模板决定一类键用到哪些位置，没用到的位置不占任何开销。 */
public enum KeyFaceSlot {
    /** 键面中央的主标签。 */
    MAIN,
    /** 右上角：字母键角标、九键数字、功能键的长按提示。 */
    TOP_RIGHT,
    /** 底边居中：双拼提示、九键双拼的简短提示。 */
    BOTTOM
}
