package app.msime.android.test;

import android.view.View;
import android.widget.Button;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

/** 使用产品的实际候选标签和字体布局，检验无释义、多行释义与大字号的裁切边界。 */
final class CompactCandidateHeightDeviceChecks {
    static void run(Class<?> type, Object service, float density) throws Exception {
        Field size = field(type, "candidateFontSize"), scale = field(type, "candidateScalePercent");
        int originalSize = size.getInt(service), originalScale = scale.getInt(service);
        Object candidates = field(type, "imeCandidates").get(service);
        Method height = method(candidates.getClass(), "stripHeightDp", int.class);
        Method make = method(candidates.getClass(), "makeCandidateButton", int.class);
        Method style = method(candidates.getClass(), "styleCandidateButton", Button.class);
        Method label = method(type, "candidateLabel", String.class, String.class, String.class, boolean.class);
        try {
            for (int percent : new int[] {50, 100, 200}) {
                for (int font : new int[] {18, 32}) {
                    size.setInt(service, Math.round(font * percent / 100f));
                    scale.setInt(service, percent);
                    for (int rows : new int[] {0, 1, 2}) {
                        int dp = (Integer) height.invoke(candidates, rows);
                        Button button = (Button) make.invoke(candidates, 0);
                        String annotation = rows == 0 ? "" : rows == 1 ? "合成释义" : "合成释义\n第二释义";
                        button.setText((CharSequence) label.invoke(service, "", "合成字", annotation, true));
                        button.setTextSize(android.util.TypedValue.COMPLEX_UNIT_PX,
                            size.getInt(service) * density * Math.min(1.15f,
                                button.getResources().getConfiguration().fontScale));
                        button.setLines(rows + 1);
                        button.setSelected(true);
                        style.invoke(candidates, button);
                        int pixels = Math.round(dp * density);
                        button.measure(View.MeasureSpec.makeMeasureSpec(Math.round(300 * density), View.MeasureSpec.EXACTLY),
                            View.MeasureSpec.makeMeasureSpec(pixels, View.MeasureSpec.EXACTLY));
                        button.layout(0, 0, button.getMeasuredWidth(), pixels);
                        int available = pixels - button.getCompoundPaddingTop() - button.getCompoundPaddingBottom();
                        if (button.getLayout() == null || button.getLayout().getHeight() > available)
                            throw new AssertionError("候选标签被裁切：scale=" + percent + ";font=" + font
                                + ";rows=" + rows + ";available=" + available + ";layout="
                                + (button.getLayout() == null ? -1 : button.getLayout().getHeight()));
                        for (String member : new String[] {"expandCandidates", "closePrediction"}) {
                            View action = (View) field(type, member).get(service);
                            if (action.getLayoutParams().height > pixels)
                                throw new AssertionError("候选操作超出行高：" + member);
                        }
                    }
                }
            }
        } finally {
            size.setInt(service, originalSize);
            scale.setInt(service, originalScale);
        }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field result = type.getDeclaredField(name); result.setAccessible(true); return result;
    }
    private static Method method(Class<?> type, String name, Class<?>... arguments) throws Exception {
        Method result = type.getDeclaredMethod(name, arguments); result.setAccessible(true); return result;
    }
}
