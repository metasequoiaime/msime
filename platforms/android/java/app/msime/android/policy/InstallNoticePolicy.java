package app.msime.android.policy;

/** 首次安装或安装包更新时临时说明的显示策略。 */
public final class InstallNoticePolicy {
    private static final long MIN_DELAY_MILLIS = 4000;
    private static final long MAX_DELAY_MILLIS = 30000;
    private static final long MILLIS_PER_CHARACTER = 130;

    private InstallNoticePolicy() {}

    public static boolean shouldShow(String currentInstallStamp, String shownInstallStamp) {
        return currentInstallStamp != null && !currentInstallStamp.isEmpty()
            && !currentInstallStamp.equals(shownInstallStamp);
    }

    public static long dismissDelayMillis(int messageLength) {
        int length = Math.max(0, messageLength);
        return Math.min(MAX_DELAY_MILLIS,
            Math.max(MIN_DELAY_MILLIS, MIN_DELAY_MILLIS + length * MILLIS_PER_CHARACTER));
    }
}
