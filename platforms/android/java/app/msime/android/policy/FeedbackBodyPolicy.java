package app.msime.android.policy;

/** 限制复制到 GitHub 问题单 URL 中的反馈正文。 */
public final class FeedbackBodyPolicy {
    public static final int MAX_LENGTH = 4_000;

    private FeedbackBodyPolicy() {}

    public static String clip(String value) {
        if (value == null || value.length() <= MAX_LENGTH) return value;
        int end = MAX_LENGTH;
        // 避免截断 emoji 时把孤立的高代理项带入反馈正文。
        if (end > 0 && Character.isHighSurrogate(value.charAt(end - 1))) end--;
        return value.substring(0, end);
    }
}
