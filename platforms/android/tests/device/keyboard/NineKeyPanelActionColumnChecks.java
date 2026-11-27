package app.msime.android.test;

import android.graphics.Paint;
import android.graphics.Rect;
import android.util.DisplayMetrics;
import android.util.TypedValue;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.Function;
import java.util.function.Predicate;

/**
 * 九键三栏面板右栏：五个功能键竖排一列、互不重叠，每个键放得下自己的键面；「笔画」「单字」两个开关的键面固定，面板刚打开时都没选中。
 *
 * <p>其他步骤按读屏描述找键再点，键面被切成一个字（#11283 的两排三列时每个键只剩十几 dp）也照样通过；这里按键的实际边界判断。键面按功能键下限 10 sp 量宽高，字号再大由键自己缩到下限，量不到的只有排版本身给不出的空间。
 */
final class NineKeyPanelActionColumnChecks {
    /** 从上到下的顺序；两个开关按面板刚打开（拼音、全部候选）时的读屏描述找。 */
    private static final String[] ACTIONS = {
        "收起候选面板", "候选面板 删除", "候选面板 重输", "切换到笔画筛选", "只显示单字",
    };
    /** 与 KeyboardTypographyPolicy.FUNCTION_KEY_MIN_TEXT_SP 和 KEY_CAP_HORIZONTAL_PADDING_DP 相同。 */
    private static final float MIN_TEXT_SP = 9f;
    private static final float HORIZONTAL_PADDING_DP = 2f;

    private NineKeyPanelActionColumnChecks() {}

    static void check(DeviceSmoke t,
                      Function<Predicate<AccessibilityNodeInfo>, Predicate<AccessibilityNodeInfo>> inPanel) {
        DisplayMetrics metrics = t.getTargetContext().getResources().getDisplayMetrics();
        Paint paint = new Paint();
        paint.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, MIN_TEXT_SP, metrics)
            / Math.max(1f, t.getTargetContext().getResources().getConfiguration().fontScale));
        Paint.FontMetrics font = paint.getFontMetrics();
        float minHeight = font.descent - font.ascent;
        float padding = 2 * TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, HORIZONTAL_PADDING_DP, metrics);
        Rect previous = null;
        for (String description : ACTIONS) {
            t.stage = "panel action column: " + description;
            AccessibilityNodeInfo node = t.awaitStableBounds(inPanel.apply(t.described(description)));
            Rect bounds = new Rect();
            node.getBoundsInScreen(bounds);
            CharSequence text = node.getText();
            String label = text == null ? "" : text.toString();
            float needWidth = paint.measureText(label) + padding;
            if (bounds.width() < needWidth || bounds.height() < minHeight)
                throw new AssertionError(description + " is " + bounds.width() + "x" + bounds.height()
                    + " px, label \"" + label + "\" needs at least " + Math.round(needWidth) + "x"
                    + Math.round(minHeight) + " px");
            if (previous != null) {
                if (Math.abs(bounds.left - previous.left) > 1 || Math.abs(bounds.right - previous.right) > 1)
                    throw new AssertionError(description + " " + bounds.toShortString()
                        + " is not in the same column as the key above " + previous.toShortString());
                if (bounds.top < previous.bottom)
                    throw new AssertionError(description + " " + bounds.toShortString()
                        + " overlaps or sits above the key before it " + previous.toShortString());
            }
            previous = bounds;
        }
        t.stage = "panel toggles start off with fixed faces";
        toggle(t, inPanel, "切换到笔画筛选", "笔画", false);
        toggle(t, inPanel, "只显示单字", "单字", false);
    }

    /** 开关键：键面固定是 {@code face}，选中态等于 {@code on}。 */
    static AccessibilityNodeInfo toggle(DeviceSmoke t,
                                        Function<Predicate<AccessibilityNodeInfo>, Predicate<AccessibilityNodeInfo>> inPanel,
                                        String description, String face, boolean on) {
        return t.await(inPanel.apply(t.described(description)).and(node ->
            node.getText() != null && face.equals(node.getText().toString()) && node.isSelected() == on));
    }
}
