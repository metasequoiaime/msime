package app.msime.android;

/**
 * 键组算出的键面参数，组里每个键都用这一份；位置都相对键自己的键格，所以键格比组里最小的键大的键照样居中、贴边。
 *
 * <p>绘制规则：主标签水平居中再加 {@code shiftXPx}，竖直方向让内容域墨迹（按模板只算基线以上的部分，见 {@link KeyFaceTemplate#capAligned}）的中线落在主标签区（键格去掉上方 {@code boxTopPx}、下方 {@code boxBottomInsetPx}）的中线上；右上角提示每段文字自己的墨迹右沿离右边 {@code insetXPx}，内容域墨迹的上沿离上边 {@code insetPx}；底部提示水平居中，内容域墨迹的下沿离下边 {@code insetPx}。提示为 {@code null} 表示这一组不画它。图标（{@code iconPx} 边长）在键格里居中；模板不画图标时为 0。
 */
public record KeyFaceParams(KeyFaceTemplate.Strategy strategy, Main main, Hint topRight, Hint bottom, float iconPx) {
    /**
     * 主标签：字号、水平偏移、主标签区，内容域墨迹相对基线的上下沿（上沿为负），以及是否只按基线以上的墨迹居中（{@link KeyFaceTemplate#capAligned}）。
     */
    public record Main(float sizePx, float shiftXPx, float boxTopPx, float boxBottomInsetPx, float inkTopPx,
            float inkBottomPx, boolean capAligned) {
        /** 在 {@code cellHeight} 高的键格里，主标签的基线位置。 */
        public float baseline(int cellHeight) {
            float center = (boxTopPx + cellHeight - boxBottomInsetPx) / 2f;
            float below = capAligned ? Math.min(inkBottomPx, 0) : inkBottomPx;
            return center - (inkTopPx + below) / 2f;
        }
    }

    /** 一处提示：字号、离上边或下边的距离、离右边的距离（右上角提示用）、内容域墨迹相对基线的上下沿。 */
    public record Hint(float sizePx, float insetPx, float insetXPx, float inkTopPx, float inkBottomPx) {
        public float inkHeight() { return inkBottomPx - inkTopPx; }
    }
}
