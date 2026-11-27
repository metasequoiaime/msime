package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 键帽看得见的轮廓：皮肤的键形（圆角、胶囊、卵石、票根）画在键格里去掉内缩和凸起厚度的那块面上。同一份描述既给 {@link KeyboardSkinKeyDrawable} 画键帽，也给键面排版问「这里还在键帽里吗」，两边不会各算各的。
 *
 * <p>形状按路径操作给出（{@link #trace}）：画键帽时接到 {@code android.graphics.Path}，排版时展平成多边形，按像素行算出键帽里能落墨的左右范围（{@link Spans}），再扣掉离轮廓的边距。键面只用这张表：右上角提示放不下时沿对角线往里挪（{@link Spans#cornerOffset}），主标签和底部提示按所在几行的范围收宽度（{@link Spans#sideInset}）。
 *
 * <p>一副键盘的键用同一个皮肤，所以一副键盘一个轮廓（{@link KeyFaceEnvironment#outline}）；表按键格尺寸缓存。只在主线程用。
 *
 * @param shape 键形
 * @param radiusPx 圆角键形的圆角半径（像素）；其他键形不用
 * @param insetPx 键帽面离键格四边的内缩（设计皮肤 1 dp）
 * @param bottomDepthPx 凸起材质在键帽面下方占的厚度（像素），面因此比键格矮
 * @param marginPx 墨迹离轮廓至少多远：边框宽度加 1 dp 抗锯齿
 */
public record KeyOutline(Shape shape, float radiusPx, float insetPx, float bottomDepthPx, float marginPx) {
    /** 方角、不内缩、不留边距：没有皮肤形状可依时（以及 JVM 冒烟的默认环境），键面只受键格限制。 */
    public static final KeyOutline NONE = new KeyOutline(Shape.ROUNDED, 0, 0, 0, 0);

    /** 皮肤里的键形名（{@code keyShape}）。 */
    public enum Shape {
        ROUNDED, CAPSULE, PEBBLE, TICKET;

        /** 偏好里的名字；不认识的按圆角。 */
        public static Shape of(String name) {
            if (name == null) return ROUNDED;
            return switch (name) {
                case "capsule" -> CAPSULE;
                case "pebble" -> PEBBLE;
                case "ticket" -> TICKET;
                default -> ROUNDED;
            };
        }
    }

    /** 接收路径操作；角度按 Android 的约定（度，y 向下，正方向顺时针）。 */
    public interface PathSink {
        void moveTo(float x, float y);

        void lineTo(float x, float y);

        void cubicTo(float x1, float y1, float x2, float y2, float x3, float y3);

        /** 从当前点连线到椭圆弧的起点再画弧（{@code Path.arcTo(oval, start, sweep)}）。 */
        void arcTo(float left, float top, float right, float bottom, float startDegrees, float sweepDegrees);

        /** 一整个圆角矩形（{@code Path.addRoundRect}，顺时针）；半径超出边长的一半时按 Skia 的规则等比缩小。 */
        void roundRect(float left, float top, float right, float bottom, float radiusX, float radiusY);

        void close();
    }

    public KeyOutline {
        if (shape == null || !(radiusPx >= 0) || !(insetPx >= 0) || !(bottomDepthPx >= 0) || !(marginPx >= 0))
            throw new IllegalArgumentException("bad outline " + shape + " " + radiusPx + " " + insetPx + " "
                + bottomDepthPx + " " + marginPx);
    }

    /**
     * 按皮肤定轮廓，与 {@code ImeStyler} 给键配的键帽一致：设计皮肤由 {@link KeyboardSkinKeyDrawable} 画（键形、内缩 1 dp、凸起 3 dp），其余皮肤是圆角半径为皮肤圆角的普通圆角矩形（不内缩）。
     */
    public static KeyOutline forSkin(KeyboardSkin skin, float density) {
        if (skin == null) return NONE;
        float margin = (float) Math.max(0, skin.borderWidth()) * density + density;
        if (skin.designed()) {
            return new KeyOutline(Shape.of(skin.keyShape()), (float) Math.max(0, skin.cornerRadius()) * density,
                KeyboardSkinKeyDrawable.FACE_INSET_DP * density,
                "raised".equals(skin.keyMaterial()) ? KeyboardSkinKeyDrawable.RAISED_DEPTH_DP * density : 0, margin);
        }
        float radius = skin.cornerRadius() <= 0 ? 0 : Math.max(1, Math.round((float) skin.cornerRadius() * density));
        return new KeyOutline(Shape.ROUNDED, radius, 0, 0, margin);
    }

    /** 方角、不内缩、不留边距：整个键格都能落墨。 */
    public boolean none() { return equals(NONE); }

    /**
     * 在键帽面 {@code left..right × top..bottom} 上描出键形。这里的每个系数都是键帽的设计本身（与 {@code packages/ui} 的 {@code keyboardKeyPath} 同源的卵石和票根），改形状只改这里。
     */
    public void trace(float left, float top, float right, float bottom, PathSink sink) {
        float width = right - left, height = bottom - top;
        switch (shape) {
            case CAPSULE -> sink.roundRect(left, top, right, bottom, height / 2, height / 2);
            case PEBBLE -> {
                sink.moveTo(left + width * .35f, top);
                sink.cubicTo(left + width * .83f, top, right, top + height * .04f, right, top + height * .3f);
                sink.cubicTo(right, top + height * .85f, left + width * .9f, bottom, left + width * .68f, bottom);
                sink.cubicTo(left + width * .18f, bottom, left, top + height * .97f, left, top + height * .7f);
                sink.cubicTo(left, top + height * .2f, left + width * .06f, top, left + width * .35f, top);
                sink.close();
            }
            case TICKET -> {
                float notch = Math.min(width, height) * .12f;
                float middle = (top + bottom) / 2;
                sink.moveTo(left, top);
                sink.lineTo(right, top);
                sink.lineTo(right, middle - notch);
                sink.arcTo(right - notch, middle - notch, right + notch, middle + notch, -90, -180);
                sink.lineTo(right, bottom);
                sink.lineTo(left, bottom);
                sink.lineTo(left, middle + notch);
                sink.arcTo(left - notch, middle - notch, left + notch, middle + notch, 90, -180);
                sink.close();
            }
            case ROUNDED -> {
                float radius = Math.min(radiusPx, Math.min(width, height) / 2);
                sink.roundRect(left, top, right, bottom, radius, radius);
            }
        }
    }

    /** {@code width}×{@code height} 像素的键格里，键帽里能落墨的范围（已扣掉边距）。 */
    public Spans spans(int width, int height) {
        SpanKey key = new SpanKey(this, width, height);
        Spans spans = CACHE.get(key);
        if (spans == null) {
            spans = Spans.of(this, width, height);
            CACHE.put(key, spans);
            if (CACHE.size() > CACHE_SIZE) CACHE.remove(CACHE.keySet().iterator().next());
        }
        return spans;
    }

    private record SpanKey(KeyOutline outline, int width, int height) { }

    private static final int CACHE_SIZE = 32;
    private static final Map<SpanKey, Spans> CACHE = new LinkedHashMap<>(CACHE_SIZE * 2, 0.75f, true);

    /**
     * 每个像素行（行中线 {@code row + 0.5}）键帽里能落墨的左右边界，已经扣掉边距：一个点算在里面，当且仅当以它为圆心、边距为半径的圆整个落在轮廓里。没有墨可落的行左边界大于右边界。
     */
    public static final class Spans {
        private static final float EPSILON = 0.01f;
        private final int width;
        private final float[] left;
        private final float[] right;

        private Spans(int width, float[] left, float[] right) {
            this.width = width;
            this.left = left;
            this.right = right;
        }

        static Spans of(KeyOutline outline, int width, int height) {
            int rows = Math.max(0, height);
            float[] left = new float[rows];
            float[] right = new float[rows];
            if (outline.none()) {
                for (int row = 0; row < rows; row++) {
                    left[row] = 0;
                    right[row] = width;
                }
                return new Spans(width, left, right);
            }
            Polygon polygon = new Polygon();
            float inset = outline.insetPx;
            outline.trace(inset, inset, width - inset, height - inset - outline.bottomDepthPx, polygon);
            float[] rawLeft = new float[rows];
            float[] rawRight = new float[rows];
            for (int row = 0; row < rows; row++) polygon.span(row + .5f, rawLeft, rawRight, row);
            float margin = outline.marginPx;
            int reach = (int) Math.ceil(margin);
            for (int row = 0; row < rows; row++) {
                float low = Float.NEGATIVE_INFINITY, high = Float.POSITIVE_INFINITY;
                for (int other = row - reach; other <= row + reach; other++) {
                    float dy = Math.abs(other - row);
                    if (dy > margin) continue;
                    if (other < 0 || other >= rows || rawLeft[other] > rawRight[other]) {
                        low = Float.POSITIVE_INFINITY;
                        high = Float.NEGATIVE_INFINITY;
                        break;
                    }
                    float chord = (float) Math.sqrt(Math.max(0, margin * margin - dy * dy));
                    low = Math.max(low, rawLeft[other] + chord);
                    high = Math.min(high, rawRight[other] - chord);
                }
                left[row] = low;
                right[row] = high;
            }
            return new Spans(width, left, right);
        }

        public int rows() { return left.length; }

        /** 第 {@code row} 行能落墨的左边界；没有时为正无穷。 */
        public float left(int row) { return row < 0 || row >= left.length ? Float.POSITIVE_INFINITY : left[row]; }

        /** 第 {@code row} 行能落墨的右边界；没有时为负无穷。 */
        public float right(int row) { return row < 0 || row >= right.length ? Float.NEGATIVE_INFINITY : right[row]; }

        /** 框 {@code x0..x1 × y0..y1}（像素）覆盖的每一行都在键帽里。 */
        public boolean fits(float x0, float y0, float x1, float y1) {
            int first = (int) Math.floor(y0), last = (int) Math.ceil(y1) - 1;
            if (first < 0 || last >= left.length || last < first) return false;
            for (int row = first; row <= last; row++)
                if (x0 < left[row] - EPSILON || x1 > right[row] + EPSILON) return false;
            return true;
        }

        /** {@code y0..y1} 这几行里，键帽两侧最多比键格收进来多少（左右取大者），水平居中的东西两边都要让出这么多；有一行落不了墨时为键格宽的一半。 */
        public float sideInset(float y0, float y1) {
            int first = Math.max(0, (int) Math.floor(y0)), last = Math.min(left.length - 1, (int) Math.ceil(y1) - 1);
            float inset = 0;
            for (int row = first; row <= last; row++) {
                if (left[row] > right[row]) return width / 2f;
                inset = Math.max(inset, Math.max(left[row], width - right[row]));
            }
            return inset;
        }

        /**
         * 右上角的框（右沿在 {@code right}、上沿在 {@code top}，{@code boxWidth}×{@code boxHeight}）要沿对角线往左下挪多少像素才整个落在键帽里；不用挪时为 0，挪到键格短边的一半还放不下时为 {@link Float#NaN}（这一处提示就不画）。
         */
        public float cornerOffset(float right, float top, float boxWidth, float boxHeight) {
            float limit = Math.min(width, left.length) / 2f;
            for (float offset = 0; offset <= limit; offset += .5f)
                if (fits(right - offset - boxWidth, top + offset, right - offset, top + offset + boxHeight)) return offset;
            return Float.NaN;
        }
    }

    /** 把路径展平成一个多边形：曲线和弧按固定段数切成折线（折线在曲线内侧，误差只会让范围略窄），精度远小于边距。 */
    private static final class Polygon implements PathSink {
        private static final int CURVE_STEPS = 24;
        private static final int QUARTER_STEPS = 16;
        private final List<float[]> points = new ArrayList<>();
        private float x;
        private float y;

        @Override public void moveTo(float x, float y) { add(x, y); }

        @Override public void lineTo(float x, float y) { add(x, y); }

        @Override public void cubicTo(float x1, float y1, float x2, float y2, float x3, float y3) {
            float x0 = x, y0 = y;
            for (int step = 1; step <= CURVE_STEPS; step++) {
                float t = step / (float) CURVE_STEPS, u = 1 - t;
                add(u * u * u * x0 + 3 * u * u * t * x1 + 3 * u * t * t * x2 + t * t * t * x3,
                    u * u * u * y0 + 3 * u * u * t * y1 + 3 * u * t * t * y2 + t * t * t * y3);
            }
        }

        @Override public void arcTo(float left, float top, float right, float bottom, float startDegrees,
                float sweepDegrees) {
            float cx = (left + right) / 2, cy = (top + bottom) / 2, rx = (right - left) / 2, ry = (bottom - top) / 2;
            int steps = Math.max(1, Math.round(Math.abs(sweepDegrees) / 90f * QUARTER_STEPS));
            for (int step = 0; step <= steps; step++) {
                double angle = Math.toRadians(startDegrees + sweepDegrees * step / steps);
                add(cx + rx * (float) Math.cos(angle), cy + ry * (float) Math.sin(angle));
            }
        }

        @Override public void roundRect(float left, float top, float right, float bottom, float radiusX,
                float radiusY) {
            float width = right - left, height = bottom - top;
            // Skia（SkRRect::setRectXY）：半径超过边长的一半时，两个半径按同一个比例缩到放得下。
            float scale = 1;
            if (radiusX * 2 > width && radiusX > 0) scale = Math.min(scale, width / (radiusX * 2));
            if (radiusY * 2 > height && radiusY > 0) scale = Math.min(scale, height / (radiusY * 2));
            float rx = radiusX * scale, ry = radiusY * scale;
            moveTo(left + rx, top);
            lineTo(right - rx, top);
            arcTo(right - 2 * rx, top, right, top + 2 * ry, -90, 90);
            lineTo(right, bottom - ry);
            arcTo(right - 2 * rx, bottom - 2 * ry, right, bottom, 0, 90);
            lineTo(left + rx, bottom);
            arcTo(left, bottom - 2 * ry, left + 2 * rx, bottom, 90, 90);
            lineTo(left, top + ry);
            arcTo(left, top, left + 2 * rx, top + 2 * ry, 180, 90);
            close();
        }

        @Override public void close() { }

        private void add(float x, float y) {
            points.add(new float[] {x, y});
            this.x = x;
            this.y = y;
        }

        /** 水平线 {@code y} 与多边形各边的交点里最左和最右的，写进第 {@code row} 格；不相交时左大于右。 */
        void span(float y, float[] left, float[] right, int row) {
            float low = Float.POSITIVE_INFINITY, high = Float.NEGATIVE_INFINITY;
            int count = points.size();
            for (int index = 0; index < count; index++) {
                float[] a = points.get(index), b = points.get((index + 1) % count);
                if ((a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y)) {
                    float crossing = a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                    low = Math.min(low, crossing);
                    high = Math.max(high, crossing);
                }
            }
            left[row] = low;
            right[row] = high;
        }
    }
}
