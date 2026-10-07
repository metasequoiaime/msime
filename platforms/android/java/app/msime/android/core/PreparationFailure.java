package app.msime.android;

/**
 * 首次准备词库失败时给用户看的一句原因。
 *
 * <p>失败原因原先只写进 logcat，设置页只说「词库准备失败」，用户的手机上出了什么错只能连电脑查。这里把异常整理成一行：去掉 Bootstrap 自己加的前缀，绝对路径换成「…」（共享层的部分错误会带上文件路径），限制长度。不涉及任何输入内容。
 */
public final class PreparationFailure {
    static final String BOOTSTRAP_PREFIX = "Shared resource verification/preparation failed: ";
    static final int MAX_LENGTH = 160;

    private PreparationFailure() {}

    public static String describe(Throwable error) {
        if (error == null) return "";
        String message = error.getMessage();
        // 最常见、用户自己能处理的一种直接说成中文：词库连同工作副本要占三百多 MB。
        if (message != null && message.contains("No space left on device"))
            return "手机存储空间不足，清理出约 400 MB 后点此重试";
        String text;
        if (message == null || message.isBlank()) {
            text = error.getClass().getSimpleName();
        } else if (message.startsWith(BOOTSTRAP_PREFIX)) {
            text = message.substring(BOOTSTRAP_PREFIX.length());
        } else {
            text = error.getClass().getSimpleName() + ": " + message;
        }
        text = TextPolicy.trimmed(text.replaceAll("/[^\\s:'\",;)]+", "…").replaceAll("\\s+", " "));
        if (text.isEmpty()) text = error.getClass().getSimpleName();
        return text.length() > MAX_LENGTH ? text.substring(0, MAX_LENGTH - 1) + "…" : text;
    }
}
