package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;

/**
 * 按键组的 {@link KeyFaceParams} 画一个键的键面。每个放进键组的键有一个，记着这个键按当前参数、文字和尺寸算好的位置；参数、文字或尺寸变了才重新定位（量这个键自己的文字），绘制时只画，不量字、不分配对象。
 *
 * <p>绘制规则见 {@link KeyFaceParams}：主标签按排版宽度水平居中（两个字母的键面分开画、中间按模板的间距），竖直方向按内容域墨迹对齐，所以同一组的主标签共用一条基线（模板要求按每个键自己的墨迹居中时例外，见 {@link KeyFaceTemplate.Glyphs}）；右上角提示按文字框靠右上角、在键帽轮廓收进去的地方挪进键帽，底部提示水平居中。
 */
final class KeyFaceRenderer {
    private final KeyFaceGroup group;
    private boolean prepared;
    private int preparedWidth;
    private int preparedHeight;
    private KeyFaceParams preparedParams;
    private String main = "";
    private String mainFirst = "";
    private String mainSecond = "";
    /** 主标签画的字号：组的字号，按自己的墨迹居中的模板里再乘这段文字的放大倍数。 */
    private float mainSize;
    private float mainX;
    private float mainSecondX;
    private float mainBaseline;
    private String corner = "";
    private float cornerX;
    private float cornerBaseline;
    private String bottom = "";
    private float bottomX;
    private float bottomBaseline;

    KeyFaceRenderer(KeyFaceGroup group) {
        this.group = group;
    }

    KeyFaceGroup group() { return group; }

    /** 参数或这个键的文字变了：下次绘制前重新定位。 */
    void stale() { prepared = false; }

    void draw(Canvas canvas, KeyboardPressButton key) {
        KeyFaceParams params = group.params();
        int width = key.getWidth();
        int height = key.getHeight();
        if (params == null || width <= 0 || height <= 0
                || !(group.measurer() instanceof PaintKeyFaceMeasurer measurer)) return;
        if (!prepared || params != preparedParams || width != preparedWidth || height != preparedHeight)
            prepare(key, measurer, params, width, height);
        KeyFaceTemplate template = group.template();
        boolean enabled = key.isEnabled();
        if (!main.isEmpty()) {
            Paint paint = paint(measurer, KeyFaceSlot.MAIN, mainSize, key.faceColor(KeyFaceSlot.MAIN),
                template, enabled);
            if (mainSecond.isEmpty()) {
                canvas.drawText(main, mainX, mainBaseline, paint);
            } else {
                canvas.drawText(mainFirst, mainX, mainBaseline, paint);
                canvas.drawText(mainSecond, mainSecondX, mainBaseline, paint);
            }
        }
        if (!corner.isEmpty()) {
            Paint paint = paint(measurer, KeyFaceSlot.TOP_RIGHT, params.topRight().sizePx(),
                key.faceColor(KeyFaceSlot.TOP_RIGHT), template, enabled);
            canvas.drawText(corner, cornerX, cornerBaseline, paint);
        }
        if (!bottom.isEmpty()) {
            Paint paint = paint(measurer, KeyFaceSlot.BOTTOM, params.bottom().sizePx(), key.faceColor(KeyFaceSlot.BOTTOM),
                template, enabled);
            canvas.drawText(bottom, bottomX, bottomBaseline, paint);
        }
    }

    private static Paint paint(PaintKeyFaceMeasurer measurer, KeyFaceSlot slot, float size, int color,
            KeyFaceTemplate template, boolean enabled) {
        Paint paint = measurer.paint(slot);
        paint.setTextSize(size);
        paint.setColor(color);
        KeyFaceTemplate.SlotSpec spec = template.slot(slot);
        paint.setAlpha(ColorPolicy.enabledAlpha(enabled, spec == null ? 255 : spec.alpha(), 96));
        return paint;
    }

    private void prepare(KeyboardPressButton key, PaintKeyFaceMeasurer measurer, KeyFaceParams params, int width,
            int height) {
        prepared = true;
        preparedParams = params;
        preparedWidth = width;
        preparedHeight = height;
        main = TextPolicy.emptyIfNull(key.faceText(KeyFaceSlot.MAIN));
        mainFirst = "";
        mainSecond = "";
        float size = params.main().sizePx();
        mainSize = size;
        float gap = group.template().pairGapEm() * size;
        KeyFaceTemplate.Glyphs glyphs = group.template().glyphs();
        mainBaseline = params.main().baseline(height);
        if (glyphs != null) {
            // 按这个键自己的墨迹居中（KeyFaceParams.Main#glyphX、#glyphBaseline），字号再乘这段文字的放大倍数。
            if (!main.isEmpty()) {
                mainSize = size * glyphs.scale(main);
                KeyFaceMeasurer.Ink ink = measurer.ink(KeyFaceSlot.MAIN, main, mainSize);
                mainX = params.main().glyphX(width, ink.left(), ink.right());
                mainBaseline = params.main().glyphBaseline(height, ink.top(), ink.bottom()) + glyphs.drop(main, size);
            }
        } else if (KeyFaceDomain.pairs(main, group.template().pairGapEm())) {
            mainFirst = main.substring(0, 1);
            mainSecond = main.substring(1);
            float first = measurer.advance(KeyFaceSlot.MAIN, mainFirst, size);
            float total = first + gap + measurer.advance(KeyFaceSlot.MAIN, mainSecond, size);
            mainX = (width - total) / 2f + params.main().shiftXPx();
            mainSecondX = mainX + first + gap;
        } else if (!main.isEmpty()) {
            mainX = (width - measurer.advance(KeyFaceSlot.MAIN, main, size)) / 2f + params.main().shiftXPx();
        }
        KeyFaceParams.Hint topRight = params.topRight();
        corner = topRight == null ? "" : TextPolicy.emptyIfNull(key.faceText(KeyFaceSlot.TOP_RIGHT));
        if (!corner.isEmpty()) {
            // 按文字框定位（排版宽度的右端、上行线），键帽轮廓在这里收进去时按这个键自己的尺寸沿对角线挪进来，挪不进来就不画。
            float hintSize = topRight.sizePx();
            cornerX = width - topRight.insetXPx() - measurer.advance(KeyFaceSlot.TOP_RIGHT, corner, hintSize);
            cornerBaseline = topRight.topBaseline();
            KeyOutline outline = group.environment().outline();
            if (!outline.none()) {
                KeyFaceMeasurer.Ink ink = measurer.ink(KeyFaceSlot.TOP_RIGHT, corner, hintSize);
                float offset = outline.spans(width, height).cornerOffset(cornerX + ink.right(), cornerBaseline + ink.top(),
                    ink.width(), ink.height());
                if (Float.isNaN(offset)) {
                    corner = "";
                } else {
                    cornerX -= offset;
                    cornerBaseline += offset;
                }
            }
        }
        KeyFaceParams.Hint under = params.bottom();
        bottom = under == null ? "" : TextPolicy.emptyIfNull(key.faceText(KeyFaceSlot.BOTTOM));
        if (!bottom.isEmpty()) {
            bottomX = (width - measurer.advance(KeyFaceSlot.BOTTOM, bottom, under.sizePx())) / 2f;
            bottomBaseline = height - under.insetPx() - under.inkBottomPx();
        }
    }
}
