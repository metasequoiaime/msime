package app.msime.android;

/** Bounds transient Engine diagnostics before they reach the keyboard surface. */
public final class InputDiagnosticPolicy {
    public static final long DISMISS_DELAY_MILLIS = 4_000L;
    public static final int MAX_LENGTH = 1_024;

    private InputDiagnosticPolicy() {}

    /**
     * Engine 的诊断原样来自 C++ 引擎，是英文（crates/engine/src/diagnostics.rs）。按键结果里的「未能保存学习或调频」用户做不了什么，输入本身已经完成，不在键盘上显示；本地模式里数据库不可用的说明译成中文。其他平台照旧，Engine 的原文不改。
     */
    private static final java.util.Set<String> SILENT = java.util.Set.of(
        "English word could not be learned.",
        "Unable to persist candidate frequency adjustment.",
        "English candidate frequency could not be persisted.",
        "Unable to persist the pinned candidate.",
        "Unable to persist candidate removal.",
        "Unable to persist candidate position.",
        "Unable to persist the composed phrase.",
        "Unable to persist the selected sentence.",
        "Typo correction could not be persisted.",
        "Autocorrect preference could not be persisted.",
        "Unable to persist the pick transition.",
        "Unable to persist personal context.",
        "Unable to persist nine-key candidate frequency adjustment.",
        "Unable to persist nine-key candidate removal.",
        "Unable to persist nine-key candidate position.");
    private static final java.util.Map<String, String> CHINESE = java.util.Map.of(
        "Quick phrase database is unavailable.", "快捷短语数据不可用",
        "Quick phrase database could not be queried.", "快捷短语查询失败",
        "Emoji database is unavailable.", "表情数据不可用",
        "Emoji database could not be queried.", "表情查询失败",
        "Kaomoji database is unavailable.", "颜文字数据不可用",
        "Kaomoji database could not be queried.", "颜文字查询失败",
        "Super-jianpin database is unavailable.", "超级简拼数据不可用",
        "Super-jianpin database could not be queried.", "超级简拼查询失败");

    public static String normalize(String value) {
        if (value == null) return "";
        String normalized = value.trim();
        if (normalized.isEmpty() || SILENT.contains(normalized)) return "";
        String chinese = CHINESE.get(normalized);
        if (chinese != null) return chinese;
        if (normalized.length() <= MAX_LENGTH) return normalized;
        int end = MAX_LENGTH - 1;
        // 避免截断 emoji 时把孤立的高代理项带入诊断提示。
        if (end > 0 && Character.isHighSurrogate(normalized.charAt(end - 1))) end--;
        return normalized.substring(0, end) + "…";
    }

    public static boolean visible(String value) {
        return !normalize(value).isEmpty();
    }
}
