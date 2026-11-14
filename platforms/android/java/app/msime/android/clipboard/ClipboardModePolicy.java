package app.msime.android;

import java.io.File;

/** 访客剪贴板的持久模式、存储隔离和回车上滑手势规则。 */
public final class ClipboardModePolicy {
    public enum Mode { MAIN, VISITOR }
    public enum EnterIcon { RETURN, TRASH }

    public static final String VISITOR_DIRECTORY = "clipboard-visitor";
    public static final String PREFERENCES = "android-clipboard-mode";
    public static final String VISITOR_KEY = "visitor";
    private static final float MIN_TRAVEL_DP = 32f;
    private static final float MAX_TRAVEL_KEYS = 3.0f;

    private ClipboardModePolicy() {}

    public static Mode fromVisitorFlag(boolean visitor) {
        return visitor ? Mode.VISITOR : Mode.MAIN;
    }

    public static File directory(File files, Mode mode) {
        if (files == null || mode == Mode.MAIN) return files;
        return new File(files, VISITOR_DIRECTORY);
    }

    /** 补读剪贴板的已处理身份也按模式隔离，避免主模式挡住访客模式的首次补读。 */
    public static String handledClipPreferenceKey(Mode mode) {
        return mode == Mode.VISITOR ? "handled_clip_visitor" : "handled_clip_main";
    }

    /** 访客模式隔离前旧版本共用的已处理身份键，只迁移到主模式。 */
    public static String legacyHandledClipPreferenceKey() {
        return "handled_clip";
    }

    /** 只有刚切换到访客模式时短暂显示垃圾桶，持久化模式恢复时仍显示回车。 */
    public static EnterIcon enterIcon(Mode mode, boolean transientTransition) {
        return mode == Mode.VISITOR && transientTransition ? EnterIcon.TRASH : EnterIcon.RETURN;
    }

    /** 判断回车键从按下位置向上滑动后是否切换模式。终点可越过键盘上沿，但不能滑出过远。 */
    public static boolean isValidSwipe(float startX, float startY, float endX, float endY,
            float keyWidth, float keyHeight, float keyboardHeight) {
        if (!(keyWidth > 0) || !(keyHeight > 0) || !(keyboardHeight > 0)
                || startY < 0 || startY > keyboardHeight)
            return false;
        float travel = startY - endY;
        float horizontal = Math.abs(endX - startX);
        float minTravel = Math.max(MIN_TRAVEL_DP, keyHeight);
        return travel >= minTravel && travel <= keyHeight * MAX_TRAVEL_KEYS
            && horizontal <= keyWidth && endY < startY;
    }
}
