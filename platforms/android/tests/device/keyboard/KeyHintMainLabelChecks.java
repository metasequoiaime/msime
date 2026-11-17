package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.util.TypedValue;
import android.view.ContextThemeWrapper;
import android.view.View;
import android.widget.Button;
import java.util.concurrent.atomic.AtomicReference;

/** 按真实工厂顺序开启角标再设字号，检查窄键完整显示主字母。 */
final class KeyHintMainLabelChecks {
    private KeyHintMainLabelChecks() { }

    static void verify(Instrumentation test) throws Exception {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        test.runOnMainSync(() -> {
            try {
                for (float fontScale : new float[] {1f, 1.45f, 2f}) {
                    Configuration config = new Configuration(test.getTargetContext().getResources().getConfiguration());
                    config.fontScale = fontScale;
                    Context context = new ContextThemeWrapper(test.getTargetContext().createConfigurationContext(config),
                        android.R.style.Theme_Material_Light_NoActionBar);
                    for (int width : new int[] {24, 28, 34, 62})
                        for (int height : new int[] {40, 52, 84})
                            for (String letter : new String[] {"Q", "W", "I", "M"})
                                checkFace(context, letter, width, height);
                }
            } catch (Exception | AssertionError error) { failure.set(error); }
        });
        if (failure.get() != null) throw new AssertionError("窄屏主字母：" + failure.get());
    }

    private static void checkFace(Context context, String letter, int widthDp, int heightDp) throws Exception {
        Class<?> type = context.getClassLoader().loadClass("app.msime.android.ShuangpinHintButton");
        Button key = (Button) type.getConstructor(Context.class).newInstance(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        key.setBackground(null);
        key.setText(letter);
        key.setTextColor(Color.GREEN);
        key.setTextSize(TypedValue.COMPLEX_UNIT_PX, 14 * density * scale);
        type.getMethod("setCornerHint", String.class).invoke(key, "1");
        type.getMethod("setCornerHintColor", int.class).invoke(key, Color.RED);
        key.setTextSize(TypedValue.COMPLEX_UNIT_PX, 22 * density * scale);
        int width = Math.round(widthDp * density), height = Math.round(heightDp * density);
        int heightSpec = View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY);
        // 加权行可能先探测零宽度，再按最终分配的宽度重测。
        key.measure(View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED), heightSpec);
        key.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY), heightSpec);
        key.layout(0, 0, width, height);
        Bitmap bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        try {
            key.draw(new Canvas(bitmap));
            check(key.getLayout().getWidth() > 0, "角标吃光主字母区域");
            check(key.getLayout().getLineWidth(0) <= key.getLayout().getWidth() + 1,
                "主字母超出排版宽度而被裁切");
            check(key.getTextSize() >= 12 * density * scale, "主字母缩成不可辨认的小字");
            int ink = 0;
            for (int y = 0; y < height; y++) for (int x = 0; x < width; x++) {
                int pixel = bitmap.getPixel(x, y);
                if (Color.green(pixel) > Color.red(pixel) + 30 && Color.green(pixel) > Color.blue(pixel) + 30)
                    ink++;
            }
            check(ink > 0, "主字母没有实际像素");
            type.getMethod("setCornerHint", String.class).invoke(key, "");
            check(Math.abs(key.getTextSize() - 22 * density * scale) < 1, "关闭角标没有恢复后来设置的字号");
        } catch (AssertionError error) {
            throw new AssertionError(letter + " " + widthDp + "x" + heightDp + " fontScale="
                + context.getResources().getConfiguration().fontScale + "：" + error.getMessage());
        } finally { bitmap.recycle(); }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
