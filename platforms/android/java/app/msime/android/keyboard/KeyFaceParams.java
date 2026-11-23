package app.msime.android;

/**
 * 键组算出的键面参数，组里每个键都用这一份；位置都相对键自己的键格，所以键格比组里最小的键大的键照样居中、贴边。
 *
 * <p>绘制规则：主标签水平居中再加 {@code shiftXPx}，竖直方向让内容域墨迹（按模板只算基线以上的部分，见 {@link KeyFaceTemplate#capAligned}）的中线落在主标签区（键格去掉上方 {@code boxTopPx}、下方 {@code boxBottomInsetPx}）的中线上；右上角提示按文字框定位：每段文字的排版宽度右端离右边 {@code insetXPx}，字体上行线离上边 {@code insetPx}（与原来的角标同一个位置），键帽轮廓在那里收进去时再沿对角线往左下挪进键帽（每个键按自己的尺寸算，见 {@link KeyOutline.Spans#cornerOffset}）；底部提示水平居中，内容域墨迹的下沿离下边 {@code insetPx}（已含凸起键帽的厚度）。提示为 {@code null} 表示这一组不画它。图标（{@code iconPx} 边长）在键格里居中；模板不画图标时为 0。
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

        /**
         * 按自己的墨迹居中的主标签（{@link KeyFaceTemplate.Glyphs}）：墨迹框（相对基线原点，{@code inkTop} 为负）的中点落在主标签区的中点上时的基线。内容域的框以基线为中线，所以 {@link #baseline} 就是主标签区的中线。
         */
        public float glyphBaseline(int cellHeight, float inkTop, float inkBottom) {
            return baseline(cellHeight) - (inkTop + inkBottom) / 2f;
        }

        /** 同上，水平方向：墨迹框（相对画字的起点）的中点落在键格中线再偏 {@code shiftXPx} 上时的起点。 */
        public float glyphX(int cellWidth, float inkLeft, float inkRight) {
            return cellWidth / 2f - (inkLeft + inkRight) / 2f + shiftXPx;
        }
    }

    /** 一处提示：字号、离上边或下边的距离、离右边的距离（右上角提示用）、内容域墨迹相对基线的上下沿，以及字体的上行线（相对基线，为负）。 */
    public record Hint(float sizePx, float insetPx, float insetXPx, float inkTopPx, float inkBottomPx, float ascentPx) {
        public float inkHeight() { return inkBottomPx - inkTopPx; }

        /** 右上角提示的基线离键格上边多远（上行线离上边 {@code insetPx}）。 */
        public float topBaseline() { return insetPx - ascentPx; }
    }
}
