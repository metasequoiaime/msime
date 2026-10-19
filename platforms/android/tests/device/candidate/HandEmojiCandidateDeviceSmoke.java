package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.os.Bundle;
import android.text.Spanned;
import android.text.style.ReplacementSpan;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicReference;

/** 从安装产品验证四个手势及肤色的可见性、字形分支和原文范围。 */
public final class HandEmojiCandidateDeviceSmoke extends Instrumentation {
    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        AtomicReference<Throwable> failure = new AtomicReference<>();
        runOnMainSync(() -> {
            Object service = null;
            try {
                Context context = getTargetContext();
                Class<?> type = context.getClassLoader().loadClass("app.msime.android.MSIMEInputService");
                service = type.getConstructor().newInstance();
                Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
                attach.setAccessible(true);
                attach.invoke(service, context);
                Method label = type.getDeclaredMethod("candidateLabel", String.class, String.class, String.class, boolean.class);
                label.setAccessible(true);
                int missing = 0;
                Set<Integer> pictures = new HashSet<>();
                for (int code : new int[] {0x1faf1, 0x1faf2, 0x1faf7, 0x1faf8}) {
                    for (int tone = 0; tone <= 5; tone++) {
                        String glyph = new String(Character.toChars(code))
                            + (tone == 0 ? "" : new String(Character.toChars(0x1f3fa + tone)));
                        CharSequence text = (CharSequence) label.invoke(service, "2 ", glyph, "合成释义", false);
                        check(text.toString().equals("2 " + glyph + "\n合成释义"), "原始码位或释义改变");
                        check(text instanceof Spanned, "缺少手势绘制兜底");
                        Spanned spanned = (Spanned) text;
                        ReplacementSpan[] spans = spanned.getSpans(0, text.length(), ReplacementSpan.class);
                        check(spans.length == 1, "手势 span 数量错误");
                        int start = spanned.getSpanStart(spans[0]), end = spanned.getSpanEnd(spans[0]);
                        check(start == 2 && end == 2 + glyph.length(), "手势 span 越过主文字边界");
                        for (int size : new int[] {24, 48, 72}) {
                            Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
                            paint.setTextSize(size);
                            boolean nativeGlyph = paint.hasGlyph(glyph);
                            if (!nativeGlyph && size == 24) missing++;
                            Paint.FontMetricsInt metrics = new Paint.FontMetricsInt();
                            int width = spans[0].getSize(paint, text, start, end, metrics);
                            check(width > 0 && width <= size * 2, "手势宽度异常");
                            if (nativeGlyph) check(width == (int) Math.ceil(paint.measureText(glyph)), "覆盖了系统字形宽度");
                            for (int background : new int[] {0xff202020, 0xffeeeeee}) {
                                Bitmap bitmap = Bitmap.createBitmap(160, 160, Bitmap.Config.ARGB_8888);
                                Canvas canvas = new Canvas(bitmap);
                                canvas.drawColor(background);
                                paint.setColor(background == 0xff202020 ? 0xffeeeeee : 0xff202020);
                                spans[0].draw(canvas, text, start, end, 8, 0, 100, 150, paint);
                                int pixels = 0, hash = 1;
                                for (int y = 0; y < 160; y++) for (int x = 0; x < 160; x++) {
                                    int pixel = bitmap.getPixel(x, y);
                                    if (pixel != background) pixels++;
                                    hash = hash * 31 + pixel;
                                }
                                bitmap.recycle();
                                check(pixels > size * size / 10, "手势仍为空白");
                                if (size == 48 && background == 0xff202020) pictures.add(hash);
                            }
                        }
                        String variant = glyph + "\ufe0f";
                        CharSequence varied = (CharSequence) label.invoke(service, "", variant, "", false);
                        check(varied.toString().equals(variant), "变体原文改变");
                        ReplacementSpan[] variantSpans = ((Spanned) varied).getSpans(0, varied.length(), ReplacementSpan.class);
                        check(variantSpans.length == 1 && ((Spanned) varied).getSpanEnd(variantSpans[0]) == variant.length(), "变体被拆开");
                    }
                }
                check(pictures.size() == 24, "手势或肤色图像重复");
                for (String untouched : new String[] {"普通😂", "\ud83e\udef1\u200d\ud83e\udef2",
                        "\ud83e\udef1\ud83c\udffb\u200d\ud83e\udef2\ud83c\udffc", "\ud83e\udef1\ufe0e"}) {
                    CharSequence text = (CharSequence) label.invoke(service, "", untouched, "", false);
                    check(text.toString().equals(untouched) && !(text instanceof Spanned), "普通文字或完整序列被拆开");
                }
                result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: 24 hand emoji, original Unicode, annotation boundaries, "
                    + "font sizes and light/dark rendering; missing system glyphs=" + missing + "\n");
            } catch (Exception | AssertionError error) {
                failure.set(error);
            } finally {
                if (service != null) for (Field field : service.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(field.getType())) continue;
                    try {
                        field.setAccessible(true);
                        ((ExecutorService) field.get(service)).shutdownNow();
                    } catch (ReflectiveOperationException ignored) {
                        // 隔离服务对象只清理本次测试创建的线程，不建立输入会话。
                    }
                }
            }
        });
        if (failure.get() != null) {
            Throwable error = failure.get();
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: hand emoji ("
                + (error instanceof AssertionError ? error.getMessage() : error.getClass().getSimpleName()) + ")\n");
        }
        finish(failure.get() == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
