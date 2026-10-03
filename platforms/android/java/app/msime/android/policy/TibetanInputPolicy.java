package app.msime.android;

/**
 * 本宿主如何驱动藏文方案：Engine 把按 EWTS（扩展威利转写）敲下的拉丁字母转换成藏文。
 *
 * <p>转写规则属于 Engine。威利转写区分大小写（`T` `D` `N` `Sh` `A` `I` `U` `M` `H` 等都是拼写），所以 Shift 和 Caps Lock 在这里就是真实的大小写：触屏 Shift 是字母大小写而不是语言切换，硬件键盘发送设备给出的字符，Caps Lock 生效。组字保存当前音节串的威利原文，内联标记的是视图的 `editing_text`（转换后的藏文），没有候选列表。组字时空格上屏藏文加音节点，`/` 上屏藏文加垂符，回车只上屏藏文；这三个键都由 Engine 处理（handled），宿主不再插入空格、斜杠或换行。
 */
public final class TibetanInputPolicy {
    /** `SchemeType::Tibetan`：这个方案在 `View.scheme` 和 `commit_context.scheme` 里的取值。 */
    public static final int TIBETAN_SCHEME = InputSchemeTraits.TIBETAN;

    private TibetanInputPolicy() {}

    /** Engine 是否在组藏文：藏文方案且不在专用英文下。藏文没有本地模式。 */
    public static boolean active(int scheme, boolean dedicatedEnglish) {
        return scheme == TIBETAN_SCHEME && !dedicatedEnglish;
    }
}
