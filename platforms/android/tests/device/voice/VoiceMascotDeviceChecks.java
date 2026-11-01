package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.os.Bundle;
import android.view.View;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

/** 使用产品素材与实际绘制路径验收静音、音量、识别中和角色开关回退，不打开麦克风。 */
final class VoiceMascotDeviceChecks {
    private VoiceMascotDeviceChecks() {}

    static void run(Instrumentation instrumentation) {
        Throwable[] failure = new Throwable[1];
        instrumentation.runOnMainSync(() -> {
            try {
                var context = instrumentation.getTargetContext();
                Class<?> type = context.getClassLoader().loadClass("app.msime.android.VoiceListeningView");
                View view = (View) type.getConstructor(android.content.Context.class).newInstance(context);
                Bitmap mascot;
                try (var input = context.getAssets().open("stickers/shuishan/01.webp")) {
                    mascot = BitmapFactory.decodeStream(input);
                }
                check(mascot != null, "内置人物必须能够解码");
                type.getMethod("setColors", int.class, int.class, int.class, int.class)
                    .invoke(view, 0xff94d9aa, 0xff102116, 0xfff4f4f4, 0xffaaaaaa);
                type.getMethod("setMascot", Bitmap.class).invoke(view, mascot);
                type.getMethod("setMascotEnabled", boolean.class).invoke(view, true);
                view.layout(0, 0, 600, 450);
                Bitmap silent = render(view);
                field(type, "levelShown").setFloat(view, 1);
                field(type, "levelTarget").setFloat(view, 1);
                Bitmap loud = render(view);
                save(instrumentation, silent, "silent");
                save(instrumentation, loud, "loud");
                if (android.animation.ValueAnimator.areAnimatorsEnabled()) {
                    check(red(loud) > red(silent), "真实音量值必须增加耳边红色信号");
                } else {
                    check(silent.sameAs(loud), "关闭动画时人物保持静止且没有红线");
                }
                type.getMethod("showRecognizing", String.class).invoke(view, "正在识别…");
                check(red(render(view)) == red(silent), "结束采集必须立即收起红线");
                type.getMethod("setLevel", float.class).invoke(view, 1f);
                check(red(render(view)) == red(silent), "识别阶段的迟到音量不能恢复红线");
                // 同一识别中文字状态下对比，差异只能来自指示区。
                Bitmap withMascot = render(view);
                type.getMethod("setMascotEnabled", boolean.class).invoke(view, false);
                Bitmap fallback = render(view);
                check(!fallback.sameAs(withMascot), "关闭工具栏人物后恢复麦克风");
                check(accent(fallback) > accent(withMascot), "回退必须画出原强调色圆盘");
                // 素材尚未解码或解码失败时，开启人物也必须画原麦克风圆盘，不能只剩一只耳朵。
                field(type, "levelShown").setFloat(view, 0);
                field(type, "levelTarget").setFloat(view, 0);
                type.getMethod("setMascot", Bitmap.class).invoke(view, (Object) null);
                type.getMethod("setMascotEnabled", boolean.class).invoke(view, true);
                Bitmap pending = render(view);
                type.getMethod("setMascotEnabled", boolean.class).invoke(view, false);
                check(pending.sameAs(render(view)), "人物素材缺席时必须回退到原麦克风圆盘");
                type.getMethod("setMascot", Bitmap.class).invoke(view, mascot);
                // 冻结静音，逐像素比较只量入口与位图归属，不受音量逐帧回落影响。
                field(type, "levelShown").setFloat(view, 0);
                field(type, "levelTarget").setFloat(view, 0);
                type.getMethod("setHoldMode", Bitmap.class).invoke(view, mascot);
                Bitmap held = render(view);
                type.getMethod("setMascotEnabled", boolean.class).invoke(view, true);
                Bitmap late = Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888);
                late.eraseColor(0xff00ffff);
                type.getMethod("setMascot", Bitmap.class).invoke(view, late);
                check(render(view).sameAs(held), "迟到的工具栏人物不能改写长按角色或绘制模式");
                late.recycle();
                // 同一素材、同一视口下外部入口与共用绘制器必须逐像素相同。
                Class<?> externalType = context.getClassLoader().loadClass("app.msime.android.OfflineVoiceListeningView");
                View external = (View) externalType.getConstructor(android.content.Context.class).newInstance(context);
                externalType.getMethod("setMascot", Bitmap.class).invoke(external, mascot);
                externalType.getMethod("setLevel", float.class).invoke(external, 1f);
                external.layout(0, 0, 600, 450);
                Bitmap expected = Bitmap.createBitmap(600, 450, Bitmap.Config.ARGB_8888);
                Class<?> drawingType = context.getClassLoader().loadClass("app.msime.android.VoiceMascotDrawing");
                Object drawing = drawingType.getConstructor().newInstance();
                drawingType.getMethod("draw", Canvas.class, Bitmap.class, float.class, float.class, float.class)
                    .invoke(drawing, new Canvas(expected), mascot, 600f, 450f, 1f);
                check(render(external).sameAs(expected), "外部入口必须保留原有绘制结果");
                mascot.recycle();
                checkEntryGuards(context, type);
            } catch (Throwable error) { failure[0] = error; }
        });
        Bundle result = new Bundle();
        if (failure[0] == null) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: mascot signal, silence, late volume, pending-mascot fallback, shared drawing and entry plainIndicator/hold guards\n");
            instrumentation.finish(Activity.RESULT_OK, result);
        } else {
            Throwable cause = failure[0];
            while (cause.getCause() != null) cause = cause.getCause();
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: voice mascot (" + cause.getClass().getSimpleName()
                + ")" + (cause instanceof AssertionError ? " " + cause.getMessage() : "") + "\n");
            instrumentation.finish(Activity.RESULT_CANCELED, result);
        }
    }

    /** 只验 ImeVoiceEntry 的 plainIndicator／hold 代次守卫：代次未变时不改动面板；新代次下 plainIndicator 关闭 show() 打开的工具栏人物并保留麦克风光圈，hold 关闭人物并切到长按缩略图。不覆盖 ImeBottomRow.bindSpaceCursor 是否、何时调用它们：产品视图在 :ime 进程，本 instrumentation 无法反射读取。 */
    private static void checkEntryGuards(android.content.Context context, Class<?> viewType) throws Exception {
        Class<?> serviceType = context.getClassLoader().loadClass("app.msime.android.MSIMEInputService");
        Class<?> entryType = context.getClassLoader().loadClass("app.msime.android.ImeVoiceEntry");
        var constructor = entryType.getDeclaredConstructor(serviceType);
        constructor.setAccessible(true);
        Object entry = constructor.newInstance(serviceType.getConstructor().newInstance());
        try {
            View view = (View) viewType.getConstructor(android.content.Context.class).newInstance(context);
            Method enable = viewType.getMethod("setMascotEnabled", boolean.class);
            Field enabled = field(viewType, "mascotEnabled");
            Field holdMode = field(viewType, "holdMode");
            // show() 对每次会话都先打开人物，这里直接调用守卫方法代替入口关闭它。
            enable.invoke(view, true);
            field(entryType, "listening").set(entry, view);
            field(entryType, "generation").setLong(entry, 1L);
            Method plain = entryType.getDeclaredMethod("plainIndicator", long.class);
            plain.setAccessible(true);
            plain.invoke(entry, 1L);
            check(enabled.getBoolean(view), "代次未变时 plainIndicator 不能关闭工具栏人物");
            plain.invoke(entry, 0L);
            check(!enabled.getBoolean(view) && !holdMode.getBoolean(view), "新代次下 plainIndicator 必须保留麦克风光圈");
            enable.invoke(view, true);
            field(entryType, "recognizing").setBoolean(entry, true);
            Method hold = entryType.getDeclaredMethod("hold", long.class);
            hold.setAccessible(true);
            check((Boolean) hold.invoke(entry, 0L), "新代次下 hold 必须切为长按面板");
            check(!enabled.getBoolean(view) && holdMode.getBoolean(view), "hold 必须使用独立缩略图而非工具栏人物");
        } finally {
            // Only the worker thread needs releasing; cancel() would reach service state this bare instance never initialized.
            ((java.util.concurrent.ExecutorService) field(entryType, "worker").get(entry)).shutdown();
        }
    }

    private static Bitmap render(View view) {
        Bitmap bitmap = Bitmap.createBitmap(view.getWidth(), view.getHeight(), Bitmap.Config.ARGB_8888);
        view.draw(new Canvas(bitmap));
        return bitmap;
    }

    private static void save(Instrumentation instrumentation, Bitmap bitmap, String state) throws java.io.IOException {
        java.io.File directory = instrumentation.getTargetContext().getExternalFilesDir(null);
        check(directory != null, "合成画面输出目录必须可用");
        try (var output = new java.io.FileOutputStream(new java.io.File(directory, "voice-mascot-" + state + ".png"))) {
            check(bitmap.compress(Bitmap.CompressFormat.PNG, 100, output), "合成画面必须保存完整");
        }
    }

    private static int red(Bitmap bitmap) {
        int count = 0;
        for (int y = 0; y < bitmap.getHeight(); y++) {
            for (int x = 0; x < bitmap.getWidth(); x++) {
                int color = bitmap.getPixel(x, y);
                if (android.graphics.Color.alpha(color) > 80 && android.graphics.Color.red(color) > 150
                        && android.graphics.Color.green(color) < 90 && android.graphics.Color.blue(color) < 90) count++;
            }
        }
        return count;
    }

    /** 原麦克风圆盘是 setColors 设定的强调色实心填充，内部像素恰好等于该色。 */
    private static int accent(Bitmap bitmap) {
        int count = 0;
        for (int y = 0; y < bitmap.getHeight(); y++) {
            for (int x = 0; x < bitmap.getWidth(); x++) {
                if (bitmap.getPixel(x, y) == 0xff94d9aa) count++;
            }
        }
        return count;
    }

    private static Field field(Class<?> type, String name) throws ReflectiveOperationException {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }
}
