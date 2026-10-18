import app.msime.android.SkinJobsApi;

/** AI 设计皮肤留在 Java 的规则：描述的形状和插画尺寸。生成、轮询、取消和失败的分类都在 Rust（#8201 迁移计划 5c），失败的文案见 AccountFailureTextSmoke。 */
public final class SkinJobsApiSmoke {
    public static void main(String[] arguments) throws Exception {
        check(SkinJobsApi.strictArtworkDimension(512L) == 512,
            "artwork dimensions accept JSON integers");
        check(SkinJobsApi.strictArtworkDimension(1.5d) == null,
            "artwork dimensions reject fractional JSON numbers");
        check(SkinJobsApi.strictArtworkDimension(0) == null && SkinJobsApi.strictArtworkDimension(2049) == null,
            "artwork dimensions stay within the decoded image bounds");

        // Rust 收 1–500 个字符、不含控制字符的描述；交过去之前先在本机挡住，免得把模型方案不合规和描述不合规混成同一个 ai_skin_invalid。
        check(SkinJobsApi.validPrompt("秋天的银杏"), "an ordinary description");
        check(SkinJobsApi.validPrompt("  深夜霓虹  "), "surrounding spaces are trimmed");
        check(!SkinJobsApi.validPrompt("   "), "a blank description is refused");
        check(SkinJobsApi.validPrompt("字".repeat(SkinJobsApi.MAX_PROMPT_CHARACTERS)), "500 characters");
        check(!SkinJobsApi.validPrompt("字".repeat(SkinJobsApi.MAX_PROMPT_CHARACTERS + 1)), "501 characters");
        check(SkinJobsApi.validPrompt("🌲".repeat(SkinJobsApi.MAX_PROMPT_CHARACTERS)), "characters, not UTF-16 units");
        check(!SkinJobsApi.validPrompt("两行\n描述"), "a line break is a control character");
        check(!SkinJobsApi.validPrompt(null), "no description");
        System.out.println("SkinJobsApiSmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
