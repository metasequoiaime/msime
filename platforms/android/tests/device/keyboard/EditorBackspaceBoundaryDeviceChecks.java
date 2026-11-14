package app.msime.android.test;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ReplacementSpan;
import android.view.inputmethod.InputConnection;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

/** 在 Android ICU 和安装产品上验证字素、内联对象与查询窗口的边界。 */
final class EditorBackspaceBoundaryDeviceChecks {
    private EditorBackspaceBoundaryDeviceChecks() {}

    static void run(ClassLoader loader) throws Exception {
        Class<?> policy = loader.loadClass("app.msime.android.EditorBackspaceBoundary");
        Method length = policy.getDeclaredMethod("length", CharSequence.class);
        length.setAccessible(true);
        for (String cluster : new String[] {"甲", "🙂", "👍🏽", "🇨🇳", "1️⃣", "❤️", "👩‍👩‍👧‍👦", "👨🏽‍💻", "e\u0301"}) {
            check((int) length.invoke(null, "前" + cluster) == cluster.length(), "complete last grapheme");
        }
        check((int) length.invoke(null, "🇨🇳🇯🇵") == 4, "only the last flag");
        check((int) length.invoke(null, "前[合成]") == 1, "plain brackets are not assumed to be emoji");
        Method delegate = policy.getDeclaredMethod("shouldDelegate", CharSequence.class, int.class);
        delegate.setAccessible(true);
        check((boolean) delegate.invoke(null, "前[合成]", 1), "editor owns private bracket semantics");
        check((boolean) delegate.invoke(null, "前\ufffc", 1), "editor owns object replacement semantics");
        check(!(boolean) delegate.invoke(null, "前甲", 1), "ordinary text keeps precise deletion");
        check(!(boolean) delegate.invoke(null, "前[合成]", 4), "known complete object range stays precise");
        SpannableString inline = new SpannableString("前[合成]");
        inline.setSpan(new SyntheticInlineObject(), 1, inline.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        check((int) length.invoke(null, inline) == 4, "entire inline object");
        check((int) length.invoke(null, "甲" + new String(new char[1023]).replace('\0', '\u0301')) == 0,
            "incomplete bounded cluster delegates to editor");
        StringBuilder flags = new StringBuilder();
        for (int index = 0; index < 256; index++) flags.append("🇨🇳");
        check((int) length.invoke(null, flags) == 0, "truncated flag run delegates to editor");
        check((int) length.invoke(null, "") == 0, "empty editor");
        check((int) length.invoke(null, new Object[] {null}) == 0, "unavailable context");
        Method before = policy.getDeclaredMethod("before", InputConnection.class);
        before.setAccessible(true);
        String oversized = new String(new char[2048]).replace('\0', '甲') + "乙";
        InputConnection ignoresLimit = (InputConnection) Proxy.newProxyInstance(
            InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
            (proxy, call, arguments) -> {
                if (!call.getName().equals("getTextBeforeCursor")) throw new AssertionError("Unexpected editor read");
                check((int) arguments[0] == 1024 && (int) arguments[1] == InputConnection.GET_TEXT_WITH_STYLES,
                    "bounded styled query");
                return oversized;
            });
        CharSequence bounded = (CharSequence) before.invoke(null, ignoresLimit);
        check(bounded.length() == 1024 && bounded.charAt(1023) == '乙', "editor cannot enlarge local window");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class SyntheticInlineObject extends ReplacementSpan {
        @Override public int getSize(Paint paint, CharSequence text, int start, int end, Paint.FontMetricsInt metrics) {
            return 20;
        }

        @Override public void draw(Canvas canvas, CharSequence text, int start, int end, float x,
                                   int top, int y, int bottom, Paint paint) {}
    }
}
