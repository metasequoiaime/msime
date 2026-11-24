package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Predicate;

/**
 * 按键行按格量键宽的视图接线：用应用里真实的 `KeyRowLayout`（字母行）和横向的 `KeyboardKeyArea`（底行）排一副 26 键，键带和键盘里一样的左右外边距，核对每个键都在自己的格里（格宽减外边距），份额之和相同的行同一列的键边落在同一像素上。
 *
 * <p>份额本身（字母 1、缩进 0.5、⇧/⌫ 1.5、底行合计 10）由 JVM 上的 `KeyCellEdgesSmoke`、`KeyboardActionRowSmoke` 覆盖；这里只验证视图层按这些份额量出来的位置。宽度取奇数像素，舍入不能在某一行里漏掉一个像素。经 `OptionSheetDeviceSmoke -e key-feedback row-edges` 运行，不增加 instrumentation 数量。
 */
final class KeyRowEdgeDeviceChecks {
    private static final int WIDTH = 1079;
    private static final int HALF_GAP = 8;
    private static final float[] ROW_1 = {1, 1, 1, 1, 1, 1, 1, 1, 1, 1};
    private static final float[] ROW_2 = {.5f, 1, 1, 1, 1, 1, 1, 1, 1, 1, .5f};
    private static final float[] ROW_3 = {1.5f, 1, 1, 1, 1, 1, 1, 1, 1.5f};
    private static final float[] BOTTOM = {1.5f, 1, 1, 3, 1, 1, 1.5f};
    /** 一行带外边距的固定宽度键：有固定宽度子视图的行照 `LinearLayout` 原样排。 */
    private static final int FIXED = 100;

    private final DeviceSmoke test;
    private final List<List<View>> rows = new ArrayList<>();
    private final List<float[]> spans = new ArrayList<>();
    private LinearLayout fixedRow;

    private KeyRowEdgeDeviceChecks(DeviceSmoke test) { this.test = test; }

    static void run(DeviceSmoke test) throws Exception { new KeyRowEdgeDeviceChecks(test).run(); }

    private void run() throws Exception {
        Activity activity = test.startActivitySync(new Intent().setClassName(test.getTargetContext(),
            "app.msime.android.home.HomeActivity").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK));
        try {
            onMain(() -> activity.setContentView(build(activity)));
            test.waitForIdleSync();

            test.stage = "key row edges: every key sits in its own cell, minus its margins";
            onMain(() -> {
                for (int index = 0; index < rows.size(); index++) {
                    float[] row = spans.get(index);
                    List<View> keys = rows.get(index);
                    float total = 0;
                    for (float span : row) total += span;
                    double prefix = 0;
                    for (int key = 0; key < keys.size(); key++) {
                        int left = (int) Math.round(WIDTH * prefix / total);
                        prefix += row[key];
                        int right = key == keys.size() - 1 ? WIDTH : (int) Math.round(WIDTH * prefix / total);
                        View view = keys.get(key);
                        int margin = view instanceof Button ? HALF_GAP : 0;
                        if (view.getLeft() != left + margin || view.getRight() != right - margin)
                            throw new AssertionError("row " + index + " key " + key + " at " + view.getLeft() + "–" + view.getRight()
                                + ", cell " + left + "–" + right + " with margin " + margin);
                    }
                }
            });

            test.stage = "key row edges: rows with the same total share their column edges to the pixel";
            onMain(() -> {
                // 第二排的键边在 0.5 + k 列，第三排 ⇧ 右边起在 1.5 + k，底行 123、符、。、空格、，、中/英 的边在 1.5、2.5、3.5、6.5、7.5、8.5。
                for (float column : new float[] {1.5f, 2.5f, 3.5f, 4.5f, 5.5f, 6.5f, 7.5f, 8.5f})
                    same(1, 2, column);
                for (float column : new float[] {1.5f, 2.5f, 3.5f, 6.5f, 7.5f, 8.5f})
                    same(2, 3, column);
                for (float column : new float[] {1, 2, 3, 4, 5, 6, 7, 8, 9})
                    if (edge(0, column) < 0) throw new AssertionError("row 1 has no edge at column " + column);
            });

            test.stage = "key row edges: a row with a fixed-width key keeps the stock LinearLayout layout";
            onMain(() -> {
                View first = fixedRow.getChildAt(0);
                View second = fixedRow.getChildAt(1);
                if (first.getLeft() != HALF_GAP || first.getWidth() != FIXED || second.getLeft() != FIXED + 3 * HALF_GAP)
                    throw new AssertionError("fixed row laid out at " + first.getLeft() + "/" + first.getWidth() + "/" + second.getLeft());
            });
        } finally {
            onMain(activity::finish);
        }
    }

    /** 两排在第 `column` 份处的格边（键的外沿加外边距）必须是同一像素。 */
    private void same(int rowA, int rowB, float column) {
        int a = edge(rowA, column);
        int b = edge(rowB, column);
        if (a < 0 || b < 0 || a != b)
            throw new AssertionError("rows " + rowA + " and " + rowB + " at column " + column + ": " + a + " vs " + b);
    }

    /** 第 `row` 排里前缀份额恰好是 `column` 的那条格边（像素），没有时返回 -1。 */
    private int edge(int row, float column) {
        float[] rowSpans = spans.get(row);
        List<View> keys = rows.get(row);
        float prefix = 0;
        for (int key = 0; key < keys.size(); key++) {
            prefix += rowSpans[key];
            if (Math.abs(prefix - column) < 1e-4) {
                View view = keys.get(key);
                return view.getRight() + (view instanceof Button ? HALF_GAP : 0);
            }
        }
        return -1;
    }

    private View build(Activity activity) throws Exception {
        Context context = test.getTargetContext();
        ClassLoader loader = context.getClassLoader();
        Constructor<?> rowType = loader.loadClass("app.msime.android.KeyRowLayout").getConstructor(Context.class);
        Constructor<?> areaType = loader.loadClass("app.msime.android.KeyboardKeyArea")
            .getConstructor(Context.class, Predicate.class);
        Predicate<View> spaced = view -> view instanceof Button;
        LinearLayout column = new LinearLayout(activity);
        column.setOrientation(LinearLayout.VERTICAL);
        for (float[] row : new float[][] {ROW_1, ROW_2, ROW_3})
            column.addView(row((LinearLayout) rowType.newInstance(activity), activity, row), matchWidth());
        LinearLayout bottom = (LinearLayout) areaType.newInstance(activity, spaced);
        bottom.setOrientation(LinearLayout.HORIZONTAL);
        column.addView(row(bottom, activity, BOTTOM), matchWidth());
        fixedRow = (LinearLayout) rowType.newInstance(activity);
        for (int index = 0; index < 2; index++) {
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(FIXED, 100);
            params.setMargins(HALF_GAP, 0, HALF_GAP, 0);
            fixedRow.addView(new Button(activity), params);
        }
        LinearLayout.LayoutParams weighted = new LinearLayout.LayoutParams(0, 100, 1f);
        fixedRow.addView(new View(activity), weighted);
        column.addView(fixedRow, matchWidth());
        LinearLayout root = new LinearLayout(activity);
        root.addView(column, new LinearLayout.LayoutParams(WIDTH, LinearLayout.LayoutParams.WRAP_CONTENT));
        return root;
    }

    /** 一排键：整数份额的是键（带左右外边距），半份的缩进是不带外边距的占位视图，与键盘里一样。 */
    private LinearLayout row(LinearLayout row, Activity activity, float[] weights) {
        List<View> keys = new ArrayList<>(weights.length);
        for (float weight : weights) {
            boolean indent = weight == .5f;
            View view = indent ? new View(activity) : new Button(activity);
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, 100, weight);
            if (!indent) params.setMargins(HALF_GAP, 4, HALF_GAP, 4);
            row.addView(view, params);
            keys.add(view);
        }
        rows.add(keys);
        spans.add(weights);
        return row;
    }

    private static LinearLayout.LayoutParams matchWidth() {
        return new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
    }

    private void onMain(Checked action) throws Exception {
        Throwable[] failure = {null};
        test.runOnMainSync(() -> { try { action.run(); } catch (Exception | AssertionError error) { failure[0] = error; } });
        if (failure[0] instanceof AssertionError error) throw error;
        if (failure[0] instanceof Exception error) throw error;
    }

    @FunctionalInterface private interface Checked { void run() throws Exception; }
}
