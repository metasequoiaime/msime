package app.msime.ocr.module;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/**
 * PP-OCR 的前后处理：检测输入的尺寸与归一化、DB 后处理（二值化、连通域、最小外接矩形、外扩）、阅读顺序、识别输入和 CTC 贪心解码。
 *
 * <p>只用 Java 标准库，不碰 Android 类型和 ONNX Runtime，`platforms/android/check-host.sh` 的 JVM 冒烟直接测它（`tests/ocr/OcrPipelineSmoke.java`）。四边形一律是 `float[8]`，依次为左上、右上、右下、左下四个点的 x、y。
 */
final class OcrPipeline {
    // 以下三个值取自固定提交的检测模型 inference.yml，与 resources/ocr-models-android.lock.json 的 detection 一致；scripts/fetch_ocr_models.py 构建时核对 yml，scripts/test-fetch-ocr-models.py 核对这里。
    static final float DET_THRESH = 0.2f;
    static final float DET_BOX_THRESH = 0.4f;
    static final float DET_UNCLIP_RATIO = 1.4f;
    // 长边上限。上游服务端默认按短边放大，手机上检测输入到 4000 px 时内存和耗时都不可接受；评测在 960 下中文 CER 0.71%。
    static final int DET_LIMIT_SIDE = 960;
    static final int DET_MAX_CANDIDATES = 3000;
    static final int DET_MIN_SIZE = 3;
    static final int REC_HEIGHT = 48;
    static final int REC_MIN_WIDTH = 320;
    static final int REC_MAX_WIDTH = 3200;
    // 裁出的行高至少是宽的 1.5 倍时按竖排处理：识别前逆时针转 90°，排序时按列。与上游 get_rotate_crop_image 的阈值相同。
    static final float VERTICAL_RATIO = 1.5f;
    // 检测模型按 BGR 通道顺序归一化（inference.yml 的 DecodeImage img_mode: BGR 与 NormalizeImage）。
    private static final float[] DET_MEAN = {0.485f, 0.456f, 0.406f};
    private static final float[] DET_STD = {0.229f, 0.224f, 0.225f};

    private OcrPipeline() {}

    /** 检测输入的宽高：长边不超过 {@link #DET_LIMIT_SIDE}，两边都取最接近的 32 的倍数且不小于 32。 */
    static int[] detectionSize(int width, int height) {
        if (width <= 0 || height <= 0) throw new IllegalArgumentException("ocr_image_size");
        float ratio = Math.max(width, height) > DET_LIMIT_SIDE ? (float) DET_LIMIT_SIDE / Math.max(width, height) : 1f;
        return new int[]{
            Math.max(32, Math.round(width * ratio / 32f) * 32),
            Math.max(32, Math.round(height * ratio / 32f) * 32)};
    }

    /** ARGB 像素转成 NCHW、BGR 顺序、按均值方差归一化的检测输入。 */
    static float[] detectionInput(int[] argb, int width, int height) {
        int plane = width * height;
        if (argb.length != plane) throw new IllegalArgumentException("ocr_detection_pixels");
        float[] input = new float[3 * plane];
        for (int index = 0; index < plane; index++) {
            int pixel = argb[index];
            input[index] = ((pixel & 255) / 255f - DET_MEAN[0]) / DET_STD[0];
            input[plane + index] = (((pixel >> 8) & 255) / 255f - DET_MEAN[1]) / DET_STD[1];
            input[2 * plane + index] = (((pixel >> 16) & 255) / 255f - DET_MEAN[2]) / DET_STD[2];
        }
        return input;
    }

    /**
     * DB 后处理：概率图按 {@link #DET_THRESH} 二值化，8 邻接连通域取凸包和最小外接矩形，框内平均概率低于 {@link #DET_BOX_THRESH} 的丢弃，其余按 `面积 × 外扩比 / 周长` 向外扩，再换算到原图坐标。
     *
     * <p>上游用 OpenCV 的 findContours 和 pyclipper；矩形按圆角外扩后的最小外接矩形就是四边各扩出同一距离，所以这里直接放大宽高。返回的顺序未定，阅读顺序见 {@link #readingOrder}。
     */
    static List<float[]> boxes(float[] probability, int mapWidth, int mapHeight, int width, int height) {
        int count = mapWidth * mapHeight;
        if (probability.length != count) throw new IllegalArgumentException("ocr_detection_map");
        boolean[] seen = new boolean[count];
        int[] queue = new int[count];
        ArrayList<float[]> result = new ArrayList<>();
        int candidates = 0;
        for (int start = 0; start < count && candidates < DET_MAX_CANDIDATES; start++) {
            if (seen[start] || probability[start] <= DET_THRESH) continue;
            int head = 0, tail = 0;
            queue[tail++] = start;
            seen[start] = true;
            while (head < tail) {
                int index = queue[head++];
                int x = index % mapWidth, y = index / mapWidth;
                for (int dy = -1; dy <= 1; dy++) {
                    for (int dx = -1; dx <= 1; dx++) {
                        int nx = x + dx, ny = y + dy;
                        if (nx < 0 || ny < 0 || nx >= mapWidth || ny >= mapHeight) continue;
                        int next = ny * mapWidth + nx;
                        if (!seen[next] && probability[next] > DET_THRESH) {
                            seen[next] = true;
                            queue[tail++] = next;
                        }
                    }
                }
            }
            candidates++;
            if (tail < 4) continue;
            long[] points = new long[tail];
            for (int k = 0; k < tail; k++) points[k] = ((long) (queue[k] % mapWidth) << 32) | (queue[k] / mapWidth);
            double[] rect = minAreaRect(convexHull(points));
            if (Math.min(rect[2], rect[3]) < DET_MIN_SIZE) continue;
            if (boxScore(probability, mapWidth, mapHeight, corners(rect)) < DET_BOX_THRESH) continue;
            double distance = rect[2] * rect[3] * DET_UNCLIP_RATIO / (2 * (rect[2] + rect[3]));
            double[] grown = {rect[0], rect[1], rect[2] + 2 * distance, rect[3] + 2 * distance, rect[4]};
            if (Math.min(grown[2], grown[3]) < DET_MIN_SIZE + 2) continue;
            double[][] ordered = order(corners(grown));
            float[] quad = new float[8];
            for (int k = 0; k < 4; k++) {
                quad[2 * k] = Math.min(Math.max(Math.round(ordered[k][0] / mapWidth * width), 0), width);
                quad[2 * k + 1] = Math.min(Math.max(Math.round(ordered[k][1] / mapHeight * height), 0), height);
            }
            // 与上游 filter_tag_det_res 相同：裁到图内后宽或高不超过 3 px 的框不识别。
            if ((int) distance(quad, 0, 1) <= 3 || (int) distance(quad, 0, 3) <= 3) continue;
            result.add(quad);
        }
        return result;
    }

    /** 裁出的行图宽高：取对边中较长的一条，与上游 get_rotate_crop_image 相同。 */
    static int[] cropSize(float[] quad) {
        return new int[]{
            Math.max(1, (int) Math.max(distance(quad, 0, 1), distance(quad, 2, 3))),
            Math.max(1, (int) Math.max(distance(quad, 0, 3), distance(quad, 1, 2)))};
    }

    static boolean vertical(float[] quad) {
        int[] size = cropSize(quad);
        return size[1] >= VERTICAL_RATIO * size[0];
    }

    /**
     * 阅读顺序。多数框是横排时从上到下分行、行内从左到右；多数框是竖排时按列从右到左、列内从上到下，这是中文竖排的读法。
     *
     * <p>上游 sorted_boxes 只按左上角从上到下、行内从左到右排，竖排的几列因此被读反（评测里两首竖排五言诗都是先出左列）。分行（分列）比的是四个角的平均坐标，阈值是框自身行高（列宽）的一半：上游写死的 10 px 在大分辨率照片上会把同一行拆开，而用轴对齐外框的高做阈值时，拍歪的长行外框比行高大得多，相邻两行会被并成一行再按 x 排乱。
     */
    static List<float[]> readingOrder(List<float[]> quads) {
        int verticalCount = 0;
        for (float[] quad : quads) if (vertical(quad)) verticalCount++;
        boolean columns = verticalCount * 2 > quads.size();
        // 横排的主轴是 y（分行）、次轴是 x（行内）；竖排的主轴是 x（分列，取相反数让右列在前）、次轴是 y（列内）。
        int major = columns ? 0 : 1, minor = columns ? 1 : 0;
        float sign = columns ? -1f : 1f;
        ArrayList<float[]> sorted = new ArrayList<>(quads);
        sorted.sort(Comparator.<float[]>comparingDouble(quad -> sign * center(quad, major))
            .thenComparingDouble(quad -> center(quad, minor)));
        ArrayList<float[]> ordered = new ArrayList<>(sorted.size());
        ArrayList<float[]> group = new ArrayList<>();
        float groupCenter = 0, groupExtent = 0;
        for (float[] quad : sorted) {
            float quadCenter = center(quad, major), quadExtent = cropSize(quad)[columns ? 0 : 1];
            if (!group.isEmpty() && Math.abs(quadCenter - groupCenter) >= Math.min(groupExtent, quadExtent) / 2) {
                flush(group, ordered, minor);
                group.clear();
            }
            if (group.isEmpty()) {
                groupCenter = quadCenter;
                groupExtent = quadExtent;
            }
            group.add(quad);
        }
        flush(group, ordered, minor);
        return ordered;
    }

    private static void flush(List<float[]> group, List<float[]> ordered, int minor) {
        group.sort(Comparator.comparingDouble(quad -> center(quad, minor)));
        ordered.addAll(group);
    }

    private static float center(float[] quad, int axis) {
        return (quad[axis] + quad[2 + axis] + quad[4 + axis] + quad[6 + axis]) / 4;
    }

    /** 识别输入缩放后的宽：高固定为 {@link #REC_HEIGHT}，宽按比例，不超过张量宽。 */
    static int resizedWidth(int cropWidth, int cropHeight) {
        return Math.max(1, Math.min(recognitionWidth(cropWidth, cropHeight),
            (int) Math.ceil(REC_HEIGHT * (double) cropWidth / cropHeight)));
    }

    /** 识别张量的宽：至少 {@link #REC_MIN_WIDTH}，按行图比例加宽，最多 {@link #REC_MAX_WIDTH}，右侧补零。 */
    static int recognitionWidth(int cropWidth, int cropHeight) {
        double ratio = Math.max((double) REC_MIN_WIDTH / REC_HEIGHT, (double) cropWidth / cropHeight);
        return Math.min(REC_MAX_WIDTH, (int) (REC_HEIGHT * ratio));
    }

    /** 缩放后的行图转成 NCHW、BGR 顺序、归一化到 [-1, 1] 的识别输入，右侧补零到张量宽。 */
    static float[] recognitionInput(int[] argb, int resizedWidth, int tensorWidth) {
        if (argb.length != resizedWidth * REC_HEIGHT || resizedWidth > tensorWidth)
            throw new IllegalArgumentException("ocr_recognition_pixels");
        int plane = REC_HEIGHT * tensorWidth;
        float[] input = new float[3 * plane];
        for (int y = 0; y < REC_HEIGHT; y++) {
            for (int x = 0; x < resizedWidth; x++) {
                int pixel = argb[y * resizedWidth + x], offset = y * tensorWidth + x;
                input[offset] = (pixel & 255) / 127.5f - 1f;
                input[plane + offset] = ((pixel >> 8) & 255) / 127.5f - 1f;
                input[2 * plane + offset] = ((pixel >> 16) & 255) / 127.5f - 1f;
            }
        }
        return input;
    }

    /** 字典文件每行一个字符，末尾换行；不裁剪空白，全角空格本身就是一个条目。 */
    static String[] dictionary(String text) {
        if (!text.endsWith("\n")) throw new IllegalArgumentException("ocr_dictionary");
        String[] entries = text.substring(0, text.length() - 1).split("\n", -1);
        for (String entry : entries) if (entry.isEmpty()) throw new IllegalArgumentException("ocr_dictionary");
        return entries;
    }

    /**
     * CTC 贪心解码：每个时间步取最大类别，合并相邻重复，去掉空白。类别 0 是空白，1 到 n 依次是字典条目，最后一个是空格（上游 CTCLabelDecode 的 use_space_char）。
     */
    static String decode(float[] logits, int steps, int classes, String[] dictionary) {
        if (classes != dictionary.length + 2 || logits.length != steps * classes)
            throw new IllegalArgumentException("ocr_recognition_classes");
        StringBuilder text = new StringBuilder();
        int previous = 0;
        for (int step = 0; step < steps; step++) {
            int best = 0;
            float bestValue = Float.NEGATIVE_INFINITY;
            for (int index = 0; index < classes; index++) {
                float value = logits[step * classes + index];
                if (value > bestValue) {
                    bestValue = value;
                    best = index;
                }
            }
            if (best != 0 && best != previous) text.append(best <= dictionary.length ? dictionary[best - 1] : " ");
            previous = best;
        }
        return text.toString();
    }

    static float distance(float[] quad, int a, int b) {
        return (float) Math.hypot(quad[2 * a] - quad[2 * b], quad[2 * a + 1] - quad[2 * b + 1]);
    }

    /** 单调链凸包；点打包成 `x << 32 | y`，排序即按 x 再按 y。 */
    static double[][] convexHull(long[] packed) {
        long[] points = packed.clone();
        Arrays.sort(points);
        int count = points.length;
        double[][] point = new double[count][];
        for (int index = 0; index < count; index++)
            point[index] = new double[]{points[index] >> 32, points[index] & 0xffffffffL};
        double[][] hull = new double[2 * count][];
        int size = 0;
        for (int index = 0; index < count; index++) {
            while (size >= 2 && cross(hull[size - 2], hull[size - 1], point[index]) <= 0) size--;
            hull[size++] = point[index];
        }
        for (int index = count - 2, lower = size + 1; index >= 0; index--) {
            while (size >= lower && cross(hull[size - 2], hull[size - 1], point[index]) <= 0) size--;
            hull[size++] = point[index];
        }
        return Arrays.copyOf(hull, Math.max(1, size - 1));
    }

    private static double cross(double[] origin, double[] a, double[] b) {
        return (a[0] - origin[0]) * (b[1] - origin[1]) - (a[1] - origin[1]) * (b[0] - origin[0]);
    }

    /** 旋转卡壳求最小外接矩形，返回 `{中心 x, 中心 y, 宽, 高, 角度}`；退化（共线）时取轴对齐外框。 */
    static double[] minAreaRect(double[][] hull) {
        if (hull.length < 3) {
            double x0 = Double.MAX_VALUE, y0 = Double.MAX_VALUE, x1 = -Double.MAX_VALUE, y1 = -Double.MAX_VALUE;
            for (double[] point : hull) {
                x0 = Math.min(x0, point[0]);
                y0 = Math.min(y0, point[1]);
                x1 = Math.max(x1, point[0]);
                y1 = Math.max(y1, point[1]);
            }
            return new double[]{(x0 + x1) / 2, (y0 + y1) / 2, x1 - x0, y1 - y0, 0};
        }
        double best = Double.MAX_VALUE;
        double[] result = null;
        for (int index = 0; index < hull.length; index++) {
            double[] a = hull[index], b = hull[(index + 1) % hull.length];
            double ex = b[0] - a[0], ey = b[1] - a[1], length = Math.hypot(ex, ey);
            if (length == 0) continue;
            double ux = ex / length, uy = ey / length;
            double minU = Double.MAX_VALUE, maxU = -Double.MAX_VALUE, minV = Double.MAX_VALUE, maxV = -Double.MAX_VALUE;
            for (double[] point : hull) {
                double u = point[0] * ux + point[1] * uy, v = -point[0] * uy + point[1] * ux;
                minU = Math.min(minU, u);
                maxU = Math.max(maxU, u);
                minV = Math.min(minV, v);
                maxV = Math.max(maxV, v);
            }
            double area = (maxU - minU) * (maxV - minV);
            if (area < best) {
                best = area;
                double cu = (minU + maxU) / 2, cv = (minV + maxV) / 2;
                result = new double[]{cu * ux - cv * uy, cu * uy + cv * ux, maxU - minU, maxV - minV, Math.atan2(uy, ux)};
            }
        }
        return result;
    }

    private static double[][] corners(double[] rect) {
        double cos = Math.cos(rect[4]), sin = Math.sin(rect[4]), halfWidth = rect[2] / 2, halfHeight = rect[3] / 2;
        double[][] sign = {{-1, -1}, {1, -1}, {1, 1}, {-1, 1}};
        double[][] points = new double[4][];
        for (int index = 0; index < 4; index++) {
            double u = sign[index][0] * halfWidth, v = sign[index][1] * halfHeight;
            points[index] = new double[]{rect[0] + u * cos - v * sin, rect[1] + u * sin + v * cos};
        }
        return points;
    }

    /** 上游 get_mini_boxes 的排序：按 x 分出左右两对，每对里 y 小的在上；返回左上、右上、右下、左下。 */
    private static double[][] order(double[][] points) {
        double[][] sorted = points.clone();
        Arrays.sort(sorted, Comparator.comparingDouble(point -> point[0]));
        double[] topLeft, bottomLeft, topRight, bottomRight;
        if (sorted[1][1] > sorted[0][1]) { topLeft = sorted[0]; bottomLeft = sorted[1]; }
        else { topLeft = sorted[1]; bottomLeft = sorted[0]; }
        if (sorted[3][1] > sorted[2][1]) { topRight = sorted[2]; bottomRight = sorted[3]; }
        else { topRight = sorted[3]; bottomRight = sorted[2]; }
        return new double[][]{topLeft, topRight, bottomRight, bottomLeft};
    }

    /** 框内概率均值（上游 score_mode: fast），在框的外接矩形里逐像素判定是否落在四边形内。 */
    private static double boxScore(float[] probability, int width, int height, double[][] corners) {
        double x0 = width, y0 = height, x1 = 0, y1 = 0;
        for (double[] point : corners) {
            x0 = Math.min(x0, point[0]);
            y0 = Math.min(y0, point[1]);
            x1 = Math.max(x1, point[0]);
            y1 = Math.max(y1, point[1]);
        }
        int left = (int) Math.max(0, Math.floor(x0)), right = (int) Math.min(width - 1, Math.ceil(x1));
        int top = (int) Math.max(0, Math.floor(y0)), bottom = (int) Math.min(height - 1, Math.ceil(y1));
        double sum = 0;
        int count = 0;
        for (int y = top; y <= bottom; y++) {
            for (int x = left; x <= right; x++) {
                boolean inside = true;
                for (int edge = 0; edge < 4 && inside; edge++) {
                    double[] a = corners[edge], b = corners[(edge + 1) % 4];
                    inside = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]) >= -1e-6;
                }
                if (inside) {
                    sum += probability[y * width + x];
                    count++;
                }
            }
        }
        return count == 0 ? 0 : sum / count;
    }
}
