package app.msime.android.policy;

import app.msime.android.TextPolicy;

/** 限制复制到 GitHub 问题单 URL 中的反馈正文。 */
public final class FeedbackBodyPolicy {
    public static final int MAX_LENGTH = 4_000;

    private FeedbackBodyPolicy() {}

    public static String clip(String value) {
        if (value == null || value.length() <= MAX_LENGTH) return value;
        return TextPolicy.clipSurrogateSafe(value, MAX_LENGTH);
    }
}
