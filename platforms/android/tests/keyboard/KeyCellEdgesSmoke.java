import app.msime.android.KeyCellEdges;
import java.util.Arrays;

/** 按份额切格（{@link KeyCellEdges}）：边单调、行尾不漂，份额之和相同的行在同一列的边落在同一像素上。 */
public final class KeyCellEdgesSmoke {
    /** 20 列外框下 26 键一族的三排字母和底行，单位是列。 */
    private static final float[] ROW_1 = {2, 2, 2, 2, 2, 2, 2, 2, 2, 2};
    private static final float[] ROW_2 = {1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1};
    private static final float[] ROW_3 = {3, 2, 2, 2, 2, 2, 2, 2, 3};
    private static final float[] BOTTOM = {3, 2, 2, 6, 2, 2, 3};
    /** 拼音九键：3 | 14 | 3，中间三列各 14/3。 */
    private static final float[] NINE_FRAME = {3, 14, 3};
    private static final float[] NINE_CENTRE = {14f / 3, 14f / 3, 14f / 3};

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    /** 前缀份额恰好是 {@code column} 的那条边；没有这样的边时返回 -1。 */
    private static int edgeAt(float[] spans, int[] edges, float column) {
        double prefix = 0;
        if (column == 0) return edges[0];
        for (int index = 0; index < spans.length; index++) {
            prefix += spans[index];
            if (Math.abs(prefix - column) < 1e-6) return edges[index + 1];
        }
        return -1;
    }

    private static void wellFormed(float[] spans, int width) {
        int[] edges = KeyCellEdges.edges(spans, width);
        check(edges.length == spans.length + 1, "边数 " + Arrays.toString(edges));
        check(edges[0] == 0 && edges[edges.length - 1] == width, "首尾 " + width + " " + Arrays.toString(edges));
        for (int index = 1; index < edges.length; index++)
            check(edges[index] >= edges[index - 1], "单调 " + width + " " + Arrays.toString(edges));
    }

    /** 份额之和为 20 的行，第 c 列的边都在 round(width × c / 20)，与这一行有几个键无关。 */
    private static void onFrame(float[] spans, int width, String name) {
        int[] edges = KeyCellEdges.edges(spans, width);
        double prefix = 0;
        for (int index = 0; index < spans.length; index++) {
            prefix += spans[index];
            int expected = (int) Math.round(width * prefix / 20);
            check(edges[index + 1] == expected, name + " 宽 " + width + " 第 " + (index + 1) + " 条边 "
                + edges[index + 1] + "，应为 " + expected);
        }
    }

    public static void main(String[] args) {
        // 常见手机、平板、横屏和悬浮宽度（像素），含奇数宽度。
        int[] widths = {720, 1079, 1080, 1170, 1179, 1206, 1440, 1600, 1919, 2400, 863, 501};
        for (int width : widths) {
            for (float[] row : new float[][] {ROW_1, ROW_2, ROW_3, BOTTOM, NINE_FRAME, NINE_CENTRE}) wellFormed(row, width);
            onFrame(ROW_1, width, "第一排");
            onFrame(ROW_2, width, "第二排");
            onFrame(ROW_3, width, "第三排");
            onFrame(BOTTOM, width, "底行");
            onFrame(NINE_FRAME, width, "九键外框");

            int[] second = KeyCellEdges.edges(ROW_2, width);
            int[] third = KeyCellEdges.edges(ROW_3, width);
            int[] bottom = KeyCellEdges.edges(BOTTOM, width);
            // ⇧ 的右边、⌫ 的左边与第二排的键边对齐，↵ 在 ⌫ 正下方，123 在 ⇧ 正下方。
            for (float column : new float[] {3, 5, 7, 9, 11, 13, 15, 17})
                check(edgeAt(ROW_2, second, column) == edgeAt(ROW_3, third, column), "第二、三排第 " + column + " 列 宽 " + width);
            check(edgeAt(BOTTOM, bottom, 3) == edgeAt(ROW_3, third, 3), "123 与 ⇧ 宽 " + width);
            check(edgeAt(BOTTOM, bottom, 17) == edgeAt(ROW_3, third, 17), "↵ 与 ⌫ 宽 " + width);

            // 九键中间三列的边与外框一致：中间这一块的宽度就是外框里中间那一格。
            int[] frame = KeyCellEdges.edges(NINE_FRAME, width);
            int centreWidth = frame[2] - frame[1];
            int[] centre = KeyCellEdges.edges(NINE_CENTRE, centreWidth);
            check(centre[centre.length - 1] == centreWidth, "九键中间块宽 " + width);
        }

        // 键宽是格宽减左右外边距，不会是负数。
        int[] edges = KeyCellEdges.edges(ROW_1, 1080);
        check(KeyCellEdges.keyWidth(edges, 0, 8, 8) == edges[1] - edges[0] - 16, "键宽");
        check(KeyCellEdges.keyWidth(new int[] {0, 4}, 0, 8, 8) == 0, "外边距大于格宽时键宽为 0");

        // 边界情况：没有格、只有一格、份额全为 0、份额为 0 的格（隐藏的键）。
        check(Arrays.equals(KeyCellEdges.edges(new float[0], 1080), new int[] {0}), "没有格");
        check(Arrays.equals(KeyCellEdges.edges(new float[] {5}, 1080), new int[] {0, 1080}), "只有一格");
        check(Arrays.equals(KeyCellEdges.edges(new float[] {0, 0}, 1080), new int[] {0, 0, 0}), "份额全为 0");
        check(Arrays.equals(KeyCellEdges.edges(new float[] {1, 0, 1}, 1000), new int[] {0, 500, 500, 1000}), "份额为 0 的格");
        check(Arrays.equals(KeyCellEdges.edges(new float[] {1, 1}, 0), new int[] {0, 0, 0}), "宽度为 0");

        for (float bad : new float[] {-1f, Float.NaN, Float.POSITIVE_INFINITY}) {
            boolean thrown = false;
            try {
                KeyCellEdges.edges(new float[] {1, bad}, 1080);
            } catch (IllegalArgumentException expected) {
                thrown = true;
            }
            check(thrown, "非法份额 " + bad);
        }
        boolean thrown = false;
        try {
            KeyCellEdges.edges(new float[] {1}, -1);
        } catch (IllegalArgumentException expected) {
            thrown = true;
        }
        check(thrown, "负宽度");

        System.out.println("KeyCellEdgesSmoke 通过");
    }
}
