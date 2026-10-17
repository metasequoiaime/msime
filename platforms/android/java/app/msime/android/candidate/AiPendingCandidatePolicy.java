package app.msime.android;

/**
 * 在线占位行（Engine 候选来源 12 和 15）在键盘上怎么画、怎么响应（#6888）。共享内核已经不再产出 AI 占位行（AI 回答到达时直接插在第一页最后一格），这里的处理留着，旧的库或数据里出现时仍按占位行对待。
 *
 * <p>AI 占位行等回答时是「…」，回答为空或失败后是「—」；云占位行只在共享候选里暂存，Android 候选条和展开面板都不显示它。占位行不是候选：显示出来的 AI 占位行画淡，读屏念「AI 候选等待中」或「AI 候选没有结果」而不是念省略号，长按不弹管理菜单（固定、置顶、删除 Engine 都不受理），在展开的面板里点它也不收起面板。点它本身交给 Engine，Engine 返回 handled 但不上屏。
 */
public final class AiPendingCandidatePolicy {
    /** Engine `CandidateSource::AiPending`，与 iOS `CandidateMarker.aiPendingSource` 相同。 */
    public static final int SOURCE = 12;
    /** Engine `CandidateSource::CloudPending`，云候选回答前的固定席位。 */
    public static final int CLOUD_SOURCE = 15;
    /** 占位行的不透明度；其他候选是 1。 */
    public static final float PLACEHOLDER_ALPHA = 0.45f;
    /** 等回答时占位行的文字，即共享内核以前放的「…」。 */
    public static final String WAITING = "…";

    private AiPendingCandidatePolicy() {}

    public static boolean isPlaceholder(int source) {
        return source == SOURCE;
    }

    public static boolean isCloudPending(int source) {
        return source == CLOUD_SOURCE;
    }

    /** Android 不显示云候选的内部占位行，等真实候选返回后再显示。 */
    public static boolean isVisible(int source) {
        return !isCloudPending(source);
    }

    /** 按钮会复用，所以真候选显式回到不透明。 */
    public static float alpha(int source) {
        return (isPlaceholder(source) || isCloudPending(source)) ? PLACEHOLDER_ALPHA : 1f;
    }

    /** 读屏念的占位行，与 iOS `CandidateMarker.aiPendingSpoken` 一致。 */
    public static String spoken(String text) {
        return WAITING.equals(text) ? "AI 候选等待中" : "AI 候选没有结果";
    }

    /** 候选的读屏描述：占位行念它的状态，不带释义和「长按管理」；其他候选原样返回。 */
    public static String description(int source, String prefix, String text, String suffix) {
        if (isPlaceholder(source)) return prefix + spoken(text);
        if (isCloudPending(source)) return prefix + "云候选加载中";
        return prefix + text + suffix;
    }

    /** 长按是否打开候选菜单：占位行没有可管理的词，也没有释义可插入。 */
    public static boolean offersMenu(int source) {
        return !isPlaceholder(source) && !isCloudPending(source);
    }
}
