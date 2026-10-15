package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.os.Bundle;
import android.text.Spanned;
import android.text.style.ReplacementSpan;
import android.util.TypedValue;
import android.view.ContextThemeWrapper;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicReference;

/** 用安装产品的候选标签和按钮绘制合成粗等号，覆盖缺字形旧系统及深浅配色。 */
public final class HeavyEqualsCandidateDeviceSmoke extends Instrumentation {
    private static final String SYMBOL = "\ud83d\udff0";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        AtomicReference<Object> service = new AtomicReference<>();
        AtomicReference<Throwable> failure = new AtomicReference<>();
        try {
            runOnMainSync(() -> {
                try {
                    ClassLoader loader = getTargetContext().getClassLoader();
                    Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                    service.set(type.getConstructor().newInstance());
                    Method label = type.getDeclaredMethod("candidateLabel", String.class,
                        String.class, String.class, boolean.class);
                    label.setAccessible(true);
                    ContextThemeWrapper context = new ContextThemeWrapper(getTargetContext(),
                        android.R.style.Theme_Material_Light_NoActionBar);
                    Button button = (Button) loader.loadClass("app.msime.android.KeyboardPressButton")
                        .getConstructor(android.content.Context.class).newInstance(context);
                    button.setBackground(null);
                    button.setLayoutParams(new ViewGroup.LayoutParams(200, 120));
                    button.setPadding(0, 0, 0, 0);
                    button.setGravity(Gravity.CENTER);
                    button.setTextSize(TypedValue.COMPLEX_UNIT_PX, 48);
                    button.setIncludeFontPadding(false);
                    button.setSingleLine(true);
                    // 离屏按钮没有窗口预绘制来修正横向滚动位置，按实际按钮宽度排版后再取像素。
                    button.setHorizontallyScrolling(false);
                    CharSequence plain = candidateLabel(label, service.get(), "", SYMBOL, "");
                    check(plain.toString().equals(SYMBOL), "原始候选代理对改变");
                    check(plain instanceof Spanned, "缺少候选字形兜底");
                    ReplacementSpan[] spans = ((Spanned) plain).getSpans(0, plain.length(), ReplacementSpan.class);
                    check(spans.length == 1, "候选字形兜底数量错误");
                    boolean missing = !button.getPaint().hasGlyph(SYMBOL);
                    for (int[] palette : new int[][] {
                            {0xffeeeeee, 0xff202020}, {0xff202020, 0xffeeeeee},
                            {0xff8bd3a8, 0xff202020}}) {
                        button.setTextColor(palette[0]);
                        button.setText(plain);
                        button.measure(View.MeasureSpec.makeMeasureSpec(200, View.MeasureSpec.EXACTLY),
                            View.MeasureSpec.makeMeasureSpec(120, View.MeasureSpec.EXACTLY));
                        button.layout(0, 0, 200, 120);
                        Bitmap bitmap = Bitmap.createBitmap(200, 120, Bitmap.Config.ARGB_8888);
                        Canvas canvas = new Canvas(bitmap);
                        canvas.drawColor(palette[1]);
                        button.draw(canvas);
                        int bands = 0;
                        int maxPixels = 0;
                        int foregroundPixels = 0;
                        boolean previous = false;
                        for (int y = 0; y < bitmap.getHeight(); y++) {
                            int pixels = 0;
                            for (int x = 0; x < bitmap.getWidth(); x++) {
                                int pixel = bitmap.getPixel(x, y);
                                if (pixel != palette[1]) pixels++;
                                if (pixel == palette[0]) foregroundPixels++;
                            }
                            maxPixels = Math.max(maxPixels, pixels);
                            boolean drawn = pixels > 15;
                            if (drawn && !previous) bands++;
                            previous = drawn;
                        }
                        bitmap.recycle();
                        check(missing ? bands == 2 : bands > 0,
                            "候选绘制横带数=" + bands + "；系统缺字形=" + missing
                                + "；最大横带=" + maxPixels + "；字号=" + button.getTextSize());
                        check(!missing || foregroundPixels > 20, "粗等号未使用候选当前文字颜色");
                    }
                    Paint paint = new Paint(button.getPaint());
                    paint.setStyle(Paint.Style.STROKE);
                    spans[0].getSize(paint, plain, 0, plain.length(), new Paint.FontMetricsInt());
                    Bitmap bitmap = Bitmap.createBitmap(80, 80, Bitmap.Config.ARGB_8888);
                    spans[0].draw(new Canvas(bitmap), plain, 0, plain.length(), 0, 0, 50, 80, paint);
                    bitmap.recycle();
                    check(paint.getStyle() == Paint.Style.STROKE, "绘制污染文字 Paint");
                    CharSequence annotated = candidateLabel(label, service.get(), "2 ", SYMBOL + "\ufe0f", "合成释义");
                    check(annotated.toString().equals("2 " + SYMBOL + "\ufe0f\n合成释义"), "前缀、变体或释义改变");
                    Spanned annotatedSpans = (Spanned) annotated;
                    ReplacementSpan[] replacements = annotatedSpans.getSpans(0, annotated.length(), ReplacementSpan.class);
                    check(replacements.length == 1 && annotatedSpans.getSpanStart(replacements[0]) == 2
                        && annotatedSpans.getSpanEnd(replacements[0]) == 5, "兜底跨入前缀或释义");
                    CharSequence ordinary = candidateLabel(label, service.get(), "", "等于😂", "");
                    check(ordinary.toString().equals("等于😂") && !(ordinary instanceof Spanned), "普通候选被改写");
                    result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: installed candidate label, "
                        + "light/dark/highlight rendering, original Unicode and annotation spans; system missing glyph=" + missing + "\n");
                } catch (Exception | AssertionError error) {
                    failure.set(error);
                } finally {
                    if (service.get() != null) for (Field field : service.get().getClass().getDeclaredFields()) {
                        if (!ExecutorService.class.isAssignableFrom(field.getType())) continue;
                        try {
                            field.setAccessible(true);
                            ((ExecutorService) field.get(service.get())).shutdownNow();
                        } catch (ReflectiveOperationException ignored) {
                            // 隔离服务对象不建立会话，仅清理本次测试创建的工作线程。
                        }
                    }
                }
            });
            if (failure.get() != null) throw new AssertionError(failure.get());
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            String detail = failure.get() instanceof AssertionError assertion
                ? assertion.getMessage() : failure.get() == null ? error.getClass().getSimpleName()
                    : failure.get().getClass().getSimpleName();
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: candidate glyph rendering ("
                + detail + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private static CharSequence candidateLabel(Method label, Object service, String prefix,
            String text, String annotation) throws ReflectiveOperationException {
        return (CharSequence) label.invoke(service, prefix, text, annotation, false);
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
