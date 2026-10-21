package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Rect;
import android.util.TypedValue;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import java.util.concurrent.atomic.AtomicReference;

/** 用安装产品的键帽和 Android 排版器检查三种文字的实际像素边界，只绘制合成标签。 */
final class ShuangpinKeyFaceChecks {
    private ShuangpinKeyFaceChecks() { }

    static void verify(Instrumentation test) throws Exception {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        test.runOnMainSync(() -> {
            try {
                for (float fontScale : new float[] {1f, 2f}) {
                    Configuration config = new Configuration(test.getTargetContext().getResources().getConfiguration());
                    config.fontScale = fontScale;
                    Context context = new ContextThemeWrapper(test.getTargetContext().createConfigurationContext(config),
                        android.R.style.Theme_Material_Light_NoActionBar);
                    for (int height : new int[] {40, 52, 84, 29}) {
                        if (height >= 40) checkFace(context, "ShuangpinHintButton", "W", "uang iang", "2", 28, height);
                        checkFace(context, "NineKeyDigitButton", "9", "w x y z", "", 62, height);
                        checkFace(context, "NineKeyDigitButton", "2", "零 b p m f", "", 62, height);
                    }
                }
            } catch (Exception | AssertionError error) {
                failure.set(error);
            }
        });
        if (failure.get() != null) throw new AssertionError("双拼键面几何：" + failure.get());
    }

    private static void checkFace(Context context, String type, String text, String hint,
            String corner, int widthDp, int heightDp) throws Exception {
        ClassLoader loader = context.getClassLoader();
        Class<?> keyType = loader.loadClass("app.msime.android." + type);
        Button key = (Button) keyType.getConstructor(Context.class).newInstance(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        key.setBackground(null);
        key.setGravity(Gravity.CENTER);
        key.setText(text);
        key.setTextSize(TypedValue.COMPLEX_UNIT_PX, 22 * density * scale);
        key.setTextColor(Color.GREEN);
        keyType.getMethod("setHintText", String.class).invoke(key, hint);
        keyType.getMethod("setHintColor", int.class).invoke(key, Color.BLUE);
        keyType.getMethod("setCornerHint", String.class).invoke(key, corner);
        keyType.getMethod("setCornerHintColor", int.class).invoke(key, Color.RED);
        int width = Math.round(widthDp * density);
        int height = Math.round(heightDp * density);
        key.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
        key.layout(0, 0, width, height);
        Bitmap bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        try {
            Canvas canvas = new Canvas(bitmap);
            canvas.drawColor(Color.WHITE);
            key.draw(canvas);
            Rect main = ink(bitmap, 1);
            Rect bottom = ink(bitmap, 2);
            Rect top = ink(bitmap, 0);
            check(!main.isEmpty() && !bottom.isEmpty(), "主标签或底部提示消失");
            check(main.bottom < bottom.top, "主标签和底部提示重叠");
            check(corner.isEmpty() || !top.isEmpty() && !Rect.intersects(main, top), "主标签和角标重叠");
            check(key.getLayout().getLineCount() == 1, "主标签折行");
            check(key.getLayout().getHeight() <= height - key.getCompoundPaddingTop() - key.getCompoundPaddingBottom(),
                "主标签超出可用高度");
            // 就地开关提示要恢复原字号和居中位置，不能每开一次就再缩小一轮。
            keyType.getMethod("setHintText", String.class).invoke(key, "");
            check(Math.abs(key.getTextSize() - 22 * density * scale) < 1, "关闭提示没有恢复字号");
            keyType.getMethod("setHintText", String.class).invoke(key, hint);
            check(key.getText().toString().equals(text), "提示混入节点主标签");
        } catch (AssertionError error) {
            throw new AssertionError(type + " " + widthDp + "x" + heightDp + " fontScale="
                + context.getResources().getConfiguration().fontScale + "：" + error.getMessage());
        } finally {
            bitmap.recycle();
        }
    }

    private static Rect ink(Bitmap bitmap, int channel) {
        Rect bounds = new Rect();
        for (int y = 0; y < bitmap.getHeight(); y++) for (int x = 0; x < bitmap.getWidth(); x++) {
            int pixel = bitmap.getPixel(x, y);
            int red = Color.red(pixel), green = Color.green(pixel), blue = Color.blue(pixel);
            boolean selected = switch (channel) {
                case 0 -> red > green + 30 && red > blue + 30;
                case 1 -> green > red + 30 && green > blue + 30;
                default -> blue > red + 30 && blue > green + 30;
            };
            if (selected) bounds.union(x, y, x + 1, y + 1);
        }
        return bounds;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
