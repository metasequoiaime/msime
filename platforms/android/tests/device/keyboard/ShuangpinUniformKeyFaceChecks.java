package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import java.io.File;
import java.io.FileOutputStream;
import java.util.concurrent.atomic.AtomicReference;

/** 用产品的真实键帽检查同组字母的字号与基线，截图只绘制合成键面。 */
final class ShuangpinUniformKeyFaceChecks {
    private ShuangpinUniformKeyFaceChecks() { }

    static void verify(Instrumentation test) throws Exception {
        AtomicReference<Throwable> failure = new AtomicReference<>();
        test.runOnMainSync(() -> {
            try {
                for (float fontScale : new float[] {1f, 2f}) {
                    Configuration config = new Configuration(test.getTargetContext().getResources().getConfiguration());
                    config.fontScale = fontScale;
                    Context context = new ContextThemeWrapper(test.getTargetContext().createConfigurationContext(config),
                        android.R.style.Theme_Material_Light_NoActionBar);
                    if (fontScale == 1f) example(context);
                    for (int width : new int[] {28, 34, 36, 40, 62, 72})
                        for (int height : new int[] {46, 52}) verifyGroup(context, width, height);
                }
            } catch (Exception | AssertionError error) { failure.set(error); }
        });
        if (failure.get() != null) throw new AssertionError("同组双拼字母键面：" + failure.get());
    }

    private static Button key(Context context, String label, int widthDp, int heightDp) throws Exception {
        ClassLoader loader = context.getClassLoader();
        Class<?> type = loader.loadClass("app.msime.android.ShuangpinHintButton");
        Button key = (Button) type.getConstructor(Context.class).newInstance(context);
        float density = context.getResources().getDisplayMetrics().density;
        float scale = Math.min(context.getResources().getConfiguration().fontScale, 1.15f);
        key.setGravity(Gravity.CENTER);
        key.setText(label);
        key.setTextSize(TypedValue.COMPLEX_UNIT_PX, 22 * density * scale);
        String hint = switch (label) { case "W" -> "ei"; case "A" -> "a"; case "J" -> "an"; case "B" -> "in"; default -> "an"; };
        String corner = (String) loader.loadClass("app.msime.android.LetterHintTable")
            .getMethod("hint", String.class).invoke(null, label);
        type.getMethod("setHintText", String.class).invoke(key, hint);
        type.getMethod("setCornerHint", String.class).invoke(key, corner);
        Object skin = loader.loadClass("app.msime.android.KeyboardSkin").getMethod("system", boolean.class).invoke(null, true);
        Class<?> skinType = skin.getClass();
        int foreground = Color.parseColor((String) skinType.getMethod("keyForeground").invoke(skin));
        int background = Color.parseColor((String) skinType.getMethod("keyBackground").invoke(skin));
        int hintColour = Color.parseColor((String) skinType.getMethod("hint").invoke(skin));
        GradientDrawable face = new GradientDrawable();
        face.setColor(background);
        face.setCornerRadius(((Number) skinType.getMethod("cornerRadius").invoke(skin)).floatValue() * density);
        key.setBackground(face);
        key.setTextColor(foreground);
        type.getMethod("setHintColor", int.class).invoke(key, hintColour);
        type.getMethod("setCornerHintColor", int.class).invoke(key, foreground);
        int width = Math.round(widthDp * density), height = Math.round(heightDp * density);
        key.measure(View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY));
        key.layout(0, 0, width, height);
        // TextView 自动字号在 layout 中确定，再画一帧后读取最终度量。
        Bitmap bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        try { key.draw(new Canvas(bitmap)); }
        finally { bitmap.recycle(); }
        return key;
    }

    private static void verifyGroup(Context context, int width, int height) throws Exception {
        Button reference = key(context, "A", width, height);
        for (char letter = 'B'; letter <= 'Z'; letter++) {
            Button actual = key(context, String.valueOf(letter), width, height);
            if (Math.abs(reference.getTextSize() - actual.getTextSize()) > 1
                    || Math.abs(reference.getBaseline() - actual.getBaseline()) > 1)
                throw new AssertionError(width + "x" + height + " fontScale="
                    + context.getResources().getConfiguration().fontScale + " A/" + letter
                    + " 字号=" + reference.getTextSize() + "/" + actual.getTextSize()
                    + " 基线=" + reference.getBaseline() + "/" + actual.getBaseline());
        }
    }

    private static void example(Context context) throws Exception {
        float density = context.getResources().getDisplayMetrics().density;
        int gap = Math.round(4 * density), width = Math.round(36 * density), height = Math.round(52 * density);
        Bitmap bitmap = Bitmap.createBitmap(width * 4 + gap * 5, height + gap * 2, Bitmap.Config.ARGB_8888);
        try {
            Canvas canvas = new Canvas(bitmap);
            Object skin = context.getClassLoader().loadClass("app.msime.android.KeyboardSkin")
                .getMethod("system", boolean.class).invoke(null, true);
            canvas.drawColor(Color.parseColor((String) skin.getClass().getMethod("background").invoke(skin)));
            int index = 0;
            for (String label : new String[] {"A", "W", "J", "B"}) {
                canvas.save();
                canvas.translate(gap + index++ * (width + gap), gap);
                key(context, label, 36, 52).draw(canvas);
                canvas.restore();
            }
            File file = new File(context.getExternalFilesDir(null), "shuangpin-uniform-key-faces.png");
            try (FileOutputStream output = new FileOutputStream(file)) {
                if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output))
                    throw new AssertionError("键面截图写入失败");
            }
        } finally { bitmap.recycle(); }
    }
}
