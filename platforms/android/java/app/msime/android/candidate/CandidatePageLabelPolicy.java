package app.msime.android;

/**
 * 候选栏读音行右侧的页码「1/3」：画不画、画成什么。
 *
 * <p>共享偏好 `show_candidate_page_number` 管它，缺值、null 或不是布尔值时按开，与桌面宿主和 client-core 的默认一致（#10709）。关掉只去掉这段文字，候选栏本来没有翻页箭头，滑动和展开面板照常翻看候选。
 */
public final class CandidatePageLabelPolicy {
    public static final String PREFERENCE = "show_candidate_page_number";

    private CandidatePageLabelPolicy() {}

    /** `value` 是偏好文档里这个字段的原值（`opt` 的结果）；只有布尔 false 才隐藏。 */
    public static boolean shown(Object value) {
        return JsonPolicy.strictBoolean(value, true);
    }

    /** 从零起的 `page` 和 `page_count` 来自引擎视图；关掉、没有页数或页号越界时为空串。 */
    public static String label(int page, int pageCount, boolean shown) {
        if (!shown || page < 0 || pageCount <= 0) return "";
        return (page + 1) + "/" + pageCount;
    }
}
