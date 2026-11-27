package app.msime.android;

/**
 * 云候选与 AI 候选的来源标记：画在候选字后面的哪个图形、读屏补念什么、皮肤横幅写哪个字符。
 *
 * <p>与其他宿主同一套规则：Engine 来源 2 是云候选、3 是 AI 候选，标记只供显示，不改候选身份、编号或选词。共享偏好 `candidate_cloud_badge` 关掉时只隐藏云标记；`candidate_ai_badge_style` 在机器人（robot，出厂）与小鲸鱼（whale）之间选，未知值按 robot，与 `preferences.rs` 一致。占位行（来源 12 的 AI 占位、来源 15 的云占位）不是候选，不加标记，读屏仍念 {@link AiPendingCandidatePolicy} 的状态。
 *
 * <p>候选条和展开面板画单色矢量图，着释义色，跟随皮肤（与 iOS `CandidateMarker` 相同的做法）；读屏不论图形开关都念来源，关掉的只是图形。自定义皮肤横幅只能写文字，沿用其他宿主的 emoji。
 */
public final class CandidateSourceMark {
    /** Engine `CandidateSource::CloudSuggestion`。 */
    public static final int CLOUD_SOURCE = 2;
    /** Engine `CandidateSource::AiSuggestion`。 */
    public static final int AI_SOURCE = 3;
    public static final String CLOUD_PREFERENCE = "candidate_cloud_badge";
    public static final String AI_STYLE_PREFERENCE = "candidate_ai_badge_style";
    public static final String ROBOT = "robot";
    public static final String WHALE = "whale";

    public enum Glyph { NONE, CLOUD, ROBOT, WHALE }

    private CandidateSourceMark() {}

    /** 未知或缺失的样式按出厂的 robot。 */
    public static String aiStyle(String value) {
        return WHALE.equals(value) ? WHALE : ROBOT;
    }

    /** 候选字后面画的图形。 */
    public static Glyph glyph(int source, boolean cloudBadge, String aiStyle) {
        if (source == CLOUD_SOURCE) return cloudBadge ? Glyph.CLOUD : Glyph.NONE;
        if (source == AI_SOURCE) return WHALE.equals(aiStyle(aiStyle)) ? Glyph.WHALE : Glyph.ROBOT;
        return Glyph.NONE;
    }

    /** 读屏接在候选描述后面的来源说明；与 iOS 的「云候选」「AI 候选」同词。 */
    public static String spokenSuffix(int source) {
        if (source == CLOUD_SOURCE) return "，云候选";
        if (source == AI_SOURCE) return "，AI 候选";
        return "";
    }

    /** 自定义皮肤横幅里跟在候选后面的 emoji，与 Windows、macOS、HarmonyOS 和 Linux 的字符相同。 */
    public static String bannerBadge(int source, boolean cloudBadge, String aiStyle) {
        return switch (glyph(source, cloudBadge, aiStyle)) {
            case CLOUD -> "☁️";
            case ROBOT -> "🤖";
            case WHALE -> "🐳";
            case NONE -> "";
        };
    }
}
