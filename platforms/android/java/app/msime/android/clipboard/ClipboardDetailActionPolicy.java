package app.msime.android;

/** 剪贴板详情底部「插入」「返回」操作行的水平对齐方式（#11288）。 */
public final class ClipboardDetailActionPolicy {
    public static final String LEFT = "left";
    public static final String RIGHT = "right";

    private ClipboardDetailActionPolicy() {}

    /** 不认识的值回到原来的左对齐，避免损坏的本机设置改变操作位置。 */
    public static boolean isRight(String choice) {
        return RIGHT.equals(choice);
    }

    public static String label(String choice) {
        return isRight(choice) ? "右对齐" : "左对齐";
    }
}
