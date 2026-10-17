package app.msime.android;

/** 联系人候选的来源标记与显示规则。来源值和 Engine 的 `CandidateSource::Contact` 保持一致。 */
public final class ContactCandidatePolicy {
    public static final int SOURCE = 16;

    private ContactCandidatePolicy() {}

    public static boolean isContact(int source) { return source == SOURCE; }

    /** 在候选文字前显示小人标记；提交仍使用候选对象的原始手机号。 */
    public static String displayText(int source, String text) {
        return isContact(source) ? "👤 " + text : text;
    }

    public static String description(int source, String text) {
        return isContact(source) ? "来自联系人：" + text : text;
    }
}
