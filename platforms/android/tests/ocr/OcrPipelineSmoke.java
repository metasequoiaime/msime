package app.msime.ocr.module;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** 可选 OCR 模块的前后处理（OcrPipeline）：检测尺寸与归一化、DB 后处理、阅读顺序（含竖排列序）、识别输入和 CTC 解码。全部是合成数据。 */
public final class OcrPipelineSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        detectionSize();
        detectionInput();
        boxes();
        rotatedBox();
        readingOrder();
        verticalColumns();
        recognitionInput();
        dictionaryAndDecode();
        System.out.println("Android OCR pipeline: detection sizing, DB post-processing, reading order (vertical columns right to left), recognition input and CTC decoding passed");
    }

    private static void detectionSize() {
        check(Arrays.equals(OcrPipeline.detectionSize(1400, 360), new int[]{960, 256}), "long side limited to 960, multiples of 32");
        check(Arrays.equals(OcrPipeline.detectionSize(100, 20), new int[]{96, 32}), "small images keep scale, at least 32");
        check(Arrays.equals(OcrPipeline.detectionSize(10, 10), new int[]{32, 32}), "tiny images are padded up to 32");
        check(Arrays.equals(OcrPipeline.detectionSize(2048, 1536), new int[]{960, 736}), "camera photo aspect is kept");
        try {
            OcrPipeline.detectionSize(0, 10);
            throw new AssertionError("empty image is rejected");
        } catch (IllegalArgumentException expected) { }
    }

    private static void detectionInput() {
        // 纯蓝像素：BGR 顺序下第 0 个通道是 B。
        float[] input = OcrPipeline.detectionInput(new int[]{0xff0000ff}, 1, 1);
        check(close(input[0], (1f - 0.485f) / 0.229f), "blue goes to channel 0 with the first mean and std");
        check(close(input[1], (0f - 0.456f) / 0.224f), "green goes to channel 1");
        check(close(input[2], (0f - 0.406f) / 0.225f), "red goes to channel 2");
    }

    /** 轴对齐的一行：外扩距离是 面积 × 1.4 / 周长，四个角按左上、右上、右下、左下排列。 */
    private static void boxes() {
        int width = 200, height = 100;
        float[] map = new float[width * height];
        fill(map, width, 20, 30, 120, 50, 0.9f);
        // 概率高于二值化阈值、但框内均值低于 box_thresh 的区域丢弃。
        fill(map, width, 150, 70, 190, 90, 0.3f);
        // 太小的连通域丢弃。
        fill(map, width, 5, 90, 6, 91, 0.9f);
        List<float[]> found = OcrPipeline.boxes(map, width, height, width, height);
        check(found.size() == 1, "one confident text line, got " + found.size());
        float[] quad = found.get(0);
        // 像素坐标 20..120 × 30..50：宽 100、高 20，外扩 100 × 20 × 1.4 / 240 ≈ 11.67。
        check(near(quad[0], 8) && near(quad[1], 18), "top-left corner " + Arrays.toString(quad));
        check(near(quad[2], 132) && near(quad[3], 18), "top-right corner " + Arrays.toString(quad));
        check(near(quad[4], 132) && near(quad[5], 62), "bottom-right corner " + Arrays.toString(quad));
        check(near(quad[6], 8) && near(quad[7], 62), "bottom-left corner " + Arrays.toString(quad));
        check(!OcrPipeline.vertical(quad), "a wide line is horizontal");
        // 概率图是原图缩小后的，框要换算回原图坐标。
        float[] scaled = OcrPipeline.boxes(map, width, height, 2 * width, 2 * height).get(0);
        check(near(scaled[2], 264) && near(scaled[5], 124), "boxes scale to the source image " + Arrays.toString(scaled));
        check(OcrPipeline.boxes(new float[width * height], width, height, width, height).isEmpty(), "blank map has no boxes");
    }

    /** 倾斜 30° 的一行：最小外接矩形跟着转，裁剪尺寸是外扩后的宽高。 */
    private static void rotatedBox() {
        int width = 300, height = 300;
        float[] map = new float[width * height];
        double angle = Math.toRadians(30), cos = Math.cos(angle), sin = Math.sin(angle);
        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                double u = (x - 150) * cos + (y - 150) * sin, v = -(x - 150) * sin + (y - 150) * cos;
                if (Math.abs(u) <= 80 && Math.abs(v) <= 15) map[y * width + x] = 0.9f;
            }
        }
        List<float[]> found = OcrPipeline.boxes(map, width, height, width, height);
        check(found.size() == 1, "one rotated line");
        int[] size = OcrPipeline.cropSize(found.get(0));
        double distance = 160.0 * 30 * 1.4 / (2 * (160 + 30));
        check(Math.abs(size[0] - (160 + 2 * distance)) <= 4, "rotated crop width " + Arrays.toString(size));
        check(Math.abs(size[1] - (30 + 2 * distance)) <= 4, "rotated crop height " + Arrays.toString(size));
        float[] quad = found.get(0);
        check(quad[1] < quad[7] && quad[0] < quad[2], "corners keep top-left, top-right order " + Arrays.toString(quad));
    }

    private static void readingOrder() {
        float[] firstLeft = rect(10, 14, 100, 44), firstRight = rect(200, 10, 300, 40);
        float[] secondLeft = rect(12, 80, 110, 110), secondRight = rect(205, 84, 290, 112);
        List<float[]> order = OcrPipeline.readingOrder(List.of(secondRight, firstRight, secondLeft, firstLeft));
        check(order.equals(List.of(firstLeft, firstRight, secondLeft, secondRight)),
            "horizontal lines read top to bottom, left to right within a line");
        // 同一行里两个框的上沿差得比固定 10 px 多（大字号照片），仍然算一行。
        float[] bigLeft = rect(10, 30, 400, 130), bigRight = rect(450, 10, 800, 120);
        check(OcrPipeline.readingOrder(List.of(bigRight, bigLeft)).equals(List.of(bigLeft, bigRight)),
            "one line of large text stays one line");
        // 拍歪约 3° 的两行长文本，行距只比行高多一点：轴对齐外框的高远大于行高，不能因此把两行并成一行、再按 x 把缩进的第一行排到后面。
        float[] tiltedFirst = {20, 100, 1000, 152, 1000, 192, 20, 140};
        float[] tiltedSecond = {0, 145, 1000, 197, 1000, 237, 0, 185};
        check(OcrPipeline.readingOrder(List.of(tiltedSecond, tiltedFirst)).equals(List.of(tiltedFirst, tiltedSecond)),
            "tilted lines stay in top-to-bottom order");
        check(OcrPipeline.readingOrder(new ArrayList<>()).isEmpty(), "no boxes, no lines");
    }

    /** 竖排：列从右到左，列内从上到下。上游的排序会把它读成先左列。 */
    private static void verticalColumns() {
        float[] leftColumn = rect(10, 0, 50, 300), rightTop = rect(100, 5, 140, 140), rightBottom = rect(102, 160, 138, 300);
        check(OcrPipeline.vertical(leftColumn) && OcrPipeline.vertical(rightTop), "tall boxes are vertical");
        List<float[]> order = OcrPipeline.readingOrder(List.of(leftColumn, rightBottom, rightTop));
        check(order.equals(List.of(rightTop, rightBottom, leftColumn)), "columns read right to left, top to bottom within a column");

        // 端到端：两列的概率图经 DB 后处理再排序，先出右列。
        int width = 200, height = 400;
        float[] map = new float[width * height];
        fill(map, width, 30, 40, 60, 360, 0.9f);
        fill(map, width, 130, 40, 160, 360, 0.9f);
        List<float[]> found = OcrPipeline.readingOrder(OcrPipeline.boxes(map, width, height, width, height));
        check(found.size() == 2, "two columns detected");
        check(OcrPipeline.vertical(found.get(0)) && OcrPipeline.vertical(found.get(1)), "both columns are vertical");
        check(found.get(0)[0] > found.get(1)[0], "the right column comes first " + Arrays.toString(found.get(0)));
    }

    private static void recognitionInput() {
        check(OcrPipeline.recognitionWidth(100, 48) == 320 && OcrPipeline.resizedWidth(100, 48) == 100,
            "short lines pad to 320");
        check(OcrPipeline.recognitionWidth(960, 48) == 960 && OcrPipeline.resizedWidth(960, 48) == 960,
            "long lines widen the tensor");
        check(OcrPipeline.recognitionWidth(4000, 48) == 3200 && OcrPipeline.resizedWidth(4000, 48) == 3200,
            "very long lines are capped at 3200");
        check(OcrPipeline.resizedWidth(30, 90) == 16, "height scales to 48");
        int[] pixels = new int[2 * OcrPipeline.REC_HEIGHT];
        Arrays.fill(pixels, 0xff0000ff);
        float[] input = OcrPipeline.recognitionInput(pixels, 2, 4);
        int plane = OcrPipeline.REC_HEIGHT * 4;
        check(input[0] == 1f && input[plane] == -1f && input[2 * plane] == -1f, "BGR order scaled to [-1, 1]");
        check(input[2] == 0f && input[3] == 0f, "the padding on the right stays zero");
    }

    private static void dictionaryAndDecode() {
        String[] dictionary = OcrPipeline.dictionary("a\n　\nb\n");
        check(Arrays.equals(dictionary, new String[]{"a", "　", "b"}), "full-width space is a dictionary entry");
        for (String bad : new String[]{"a\nb", "a\n\nb\n"}) {
            try {
                OcrPipeline.dictionary(bad);
                throw new AssertionError("malformed dictionary is rejected");
            } catch (IllegalArgumentException expected) { }
        }
        // 类别：0 空白，1..3 字典，4 空格。
        int classes = dictionary.length + 2;
        int[] best = {1, 1, 0, 1, 3, 4, 2, 2, 0};
        float[] logits = new float[best.length * classes];
        for (int step = 0; step < best.length; step++) logits[step * classes + best[step]] = 1f;
        check(OcrPipeline.decode(logits, best.length, classes, dictionary).equals("aab 　"),
            "repeats merge, blanks drop, the last class is a space");
        try {
            OcrPipeline.decode(logits, best.length, classes + 1, dictionary);
            throw new AssertionError("class count must match the dictionary");
        } catch (IllegalArgumentException expected) { }
    }

    private static void fill(float[] map, int width, int left, int top, int right, int bottom, float value) {
        for (int y = top; y <= bottom; y++) for (int x = left; x <= right; x++) map[y * width + x] = value;
    }

    private static float[] rect(float left, float top, float right, float bottom) {
        return new float[]{left, top, right, top, right, bottom, left, bottom};
    }

    private static boolean near(float actual, float expected) {
        return Math.abs(actual - expected) <= 1.5f;
    }

    private static boolean close(float actual, float expected) {
        return Math.abs(actual - expected) < 1e-5f;
    }
}
