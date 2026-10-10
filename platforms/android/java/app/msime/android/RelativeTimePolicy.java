package app.msime.android;

/** Shared Chinese relative-time labels for elapsed whole minutes. */
public final class RelativeTimePolicy {
    private RelativeTimePolicy() {}

    public static String minutesAgo(long minutes) {
        if (minutes < 1) return "刚刚";
        if (minutes < 60) return minutes + " 分钟前";
        long hours = minutes / 60;
        if (hours < 24) return hours + " 小时前";
        return (hours / 24) + " 天前";
    }
}
