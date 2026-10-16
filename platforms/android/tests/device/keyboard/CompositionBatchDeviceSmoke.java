package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Bundle;
import android.text.InputFilter;
import android.text.InputType;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.widget.EditText;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;

/** 从安装产品调用真实 sink，用批量结束才应用写入的连接复现 Compose 地址栏。 */
public final class CompositionBatchDeviceSmoke extends Instrumentation {
    private String stage = "initialize";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            Object service = null;
            try {
                ClassLoader loader = getTargetContext().getClassLoader();
                Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
                service = type.getConstructor().newInstance();
                Method attach = method(ContextWrapper.class, "attachBaseContext", Context.class);
                attach.invoke(service, getTargetContext());
                Fixture fixture = new Fixture(loader, type, service);
                stage = "deferred first letter";
                BufferedEditor editor = fixture.reset("", 100, false);
                check(fixture.apply(null, "n"), "first letter accepted");
                check(editor.readsInBatch == 0, "readback must follow batch flush");
                check(editor.text.equals("n") && editor.composingEnd == 1,
                    "first letter remains composing");
                check(!fixture.suppressed(), "deferred write is not a length filter");
                fixture.acknowledge(editor);
                stage = "deferred next letter";
                check(fixture.apply(null, "ni"), "second letter accepted");
                check(editor.text.equals("ni") && editor.composingEnd == 2, "composition grows");
                fixture.acknowledge(editor);
                stage = "commit then new composition";
                check(fixture.apply("你", "h"), "commit and compose accepted");
                check(editor.text.equals("你h") && editor.composingStart == 1,
                    "committed text precedes new composition");
                fixture.acknowledge(editor);
                stage = "clear composition";
                check(fixture.apply(null, ""), "clear accepted");
                check(editor.text.equals("你") && editor.composingEnd == -1, "clear removes only composition");
                fixture.acknowledge(editor);
                for (boolean losesSpan : new boolean[] {false, true}) {
                    stage = "truncated composition, lost span=" + losesSpan;
                    String prefix = syntheticText(26);
                    editor = fixture.reset(prefix, 30, losesSpan);
                    check(fixture.apply(null, "nihaonihao"), "truncated composition accepted");
                    check(editor.text.equals(prefix) && fixture.suppressed(), "truncated prefix removed");
                    check(editor.readsInBatch == 0 && editor.depth == 0, "both batches closed before reads");
                    fixture.acknowledge(editor);
                    int reads = editor.reads;
                    check(fixture.apply(null, "nihaonihaoni"), "hidden composition continues");
                    check(editor.text.equals(prefix) && editor.reads == reads, "hidden composition is not rewritten");
                    fixture.acknowledge(editor);
                    check(fixture.apply("你好你好", ""), "candidate fits actual limit");
                    check(editor.text.equals(prefix + "你好你好") && !fixture.suppressed(), "only candidate consumes limit");
                    fixture.acknowledge(editor);
                }
                stage = "zero remaining space";
                editor = fixture.reset(syntheticText(30), 30, true);
                check(fixture.apply(null, "n") && fixture.suppressed(), "zero-length filter accepted");
                check(editor.text.length() == 30, "existing text preserved");
                fixture.acknowledge(editor);
                stage = "unknown editor context";
                editor = fixture.reset("", 100, false);
                editor.unknown = true;
                check(fixture.apply(null, "n") && !fixture.suppressed(), "unknown context stays inline");
                fixture.acknowledge(editor);
                stage = "Android LengthFilter and native input connection";
                EditText limited = new EditText(getTargetContext());
                limited.setInputType(InputType.TYPE_CLASS_TEXT);
                limited.setFilters(new InputFilter[] {new InputFilter.LengthFilter(30)});
                String prefix = syntheticText(26);
                limited.setText(prefix);
                limited.setSelection(prefix.length());
                InputConnection nativeConnection = limited.onCreateInputConnection(new EditorInfo());
                check(nativeConnection != null, "native editor provides a connection");
                fixture.resetConnection(nativeConnection, prefix.length());
                check(fixture.apply(null, "nihaonihao") && fixture.suppressed(), "native filter hides truncated pinyin");
                check(limited.getText().toString().equals(prefix), "native filter cleanup preserves existing text");
                check(fixture.apply(null, "nihaonihaoni") && limited.length() == 26, "native hidden composition continues");
                check(fixture.apply("你好你好", "") && !fixture.suppressed(), "native candidate accepted");
                check(limited.getText().toString().equals(prefix + "你好你好"), "native editor fills exactly thirty characters");
                stage = "cleanup rejection";
                editor = fixture.reset(syntheticText(30), 30, true);
                editor.rejectFinish = true;
                check(!fixture.apply(null, "n") && editor.depth == 0, "cleanup failure reaches caller");
                stage = "composition rejection";
                editor = fixture.reset("", 100, false);
                editor.rejectCompose = true;
                check(!fixture.apply(null, "n") && editor.reads == 0 && editor.depth == 0,
                    "failed write skips verification");
            } catch (Exception | AssertionError error) {
                failure[0] = error;
            } finally {
                if (service != null) for (Field member : service.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                    try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                    catch (ReflectiveOperationException ignored) { /* 隔离服务未启动真实输入会话。 */ }
                }
            }
        });
        boolean passed = failure[0] == null;
        result.putString("stream", passed
            ? "MSIME_DEVICE_SMOKE_PASSED: installed composition batch flush, length fallback and selection echoes\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + failure[0].getClass().getSimpleName() + ")"
                + (failure[0] instanceof AssertionError ? ": " + failure[0].getMessage() : "") + "\n");
        finish(passed ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    private static final class Fixture {
        final Class<?> type;
        final Object service;
        final Object bridge;
        final Object tracker;
        final Object source;
        final Method sink;
        final Method apply;
        Fixture(ClassLoader loader, Class<?> type, Object service) throws Exception {
            this.type = type;
            this.service = service;
            bridge = field(type, "bridge").get(service);
            tracker = field(type, "selectionEcho").get(service);
            Class<?> sourceType = loader.loadClass("app.msime.android.TypingSource");
            source = sourceType.getEnumConstants()[0];
            sink = method(type, "sink", sourceType);
            apply = method(bridge.getClass(), "apply", loader.loadClass("app.msime.android.EditorBridge$Sink"),
                String.class, String.class);
        }
        BufferedEditor reset(String text, int limit, boolean losesSpan) throws Exception {
            BufferedEditor editor = new BufferedEditor(text, limit, losesSpan);
            resetConnection(editor.connection(), text.length());
            return editor;
        }
        void resetConnection(InputConnection connection, int cursor) throws Exception {
            field(type, "connection").set(service, connection);
            field(type, "editorCompositionSuppressed").setBoolean(service, false);
            method(tracker.getClass(), "reset", int.class, int.class).invoke(tracker, cursor, cursor);
        }
        boolean apply(String commit, String composing) throws Exception {
            return (boolean) apply.invoke(bridge, sink.invoke(service, source), commit, composing);
        }
        boolean suppressed() throws Exception { return field(type, "editorCompositionSuppressed").getBoolean(service); }
        void acknowledge(BufferedEditor editor) throws Exception {
            Method acknowledge = method(tracker.getClass(), "acknowledge", int.class, int.class, int.class, int.class);
            for (int[] echo : editor.echoes) {
                check((boolean) acknowledge.invoke(tracker, echo[0], echo[0], echo[1], echo[2]),
                    "own selection echo must not cancel candidates");
            }
            editor.echoes.clear();
        }
    }

    private static final class BufferedEditor {
        String text;
        int cursor;
        int composingStart = -1;
        int composingEnd = -1;
        int depth;
        int reads;
        int readsInBatch;
        final int limit;
        final boolean losesSpan;
        boolean unknown;
        boolean rejectCompose;
        boolean rejectFinish;
        final List<Runnable> pending = new ArrayList<>();
        final List<int[]> echoes = new ArrayList<>();
        BufferedEditor(String text, int limit, boolean losesSpan) {
            this.text = text;
            cursor = text.length();
            this.limit = limit;
            this.losesSpan = losesSpan;
        }
        InputConnection connection() {
            return (InputConnection) Proxy.newProxyInstance(InputConnection.class.getClassLoader(),
                new Class<?>[] {InputConnection.class}, (proxy, call, args) -> switch (call.getName()) {
                    case "beginBatchEdit" -> { depth++; yield true; }
                    case "endBatchEdit" -> {
                        check(depth > 0, "balanced batches");
                        if (--depth == 0) {
                            int oldCursor = cursor;
                            int oldStart = composingStart;
                            int oldEnd = composingEnd;
                            for (Runnable operation : pending) operation.run();
                            pending.clear();
                            // 系统不会重复发送选区和组字区完全未变的 updateSelection。
                            if (cursor != oldCursor || composingStart != oldStart || composingEnd != oldEnd)
                                echoes.add(new int[] {cursor, composingStart, composingEnd});
                        }
                        yield true;
                    }
                    case "setComposingText", "commitText" -> {
                        boolean compose = call.getName().equals("setComposingText");
                        if (compose && rejectCompose) yield false;
                        String value = args[0].toString();
                        enqueue(() -> replace(value, compose));
                        yield true;
                    }
                    case "finishComposingText" -> {
                        if (rejectFinish) yield false;
                        enqueue(() -> { composingStart = -1; composingEnd = -1; });
                        yield true;
                    }
                    case "deleteSurroundingText" -> {
                        int length = (int) args[0];
                        enqueue(() -> { text = text.substring(0, cursor - length) + text.substring(cursor); cursor -= length; });
                        yield true;
                    }
                    case "getTextBeforeCursor" -> {
                        reads++;
                        if (depth > 0) readsInBatch++;
                        yield unknown ? null : text.substring(Math.max(0, cursor - (int) args[0]), cursor);
                    }
                    default -> throw new AssertionError("Unexpected editor operation: " + call.getName());
                });
        }
        void enqueue(Runnable operation) { if (depth == 0) operation.run(); else pending.add(operation); }
        void replace(String value, boolean compose) {
            int start = composingStart < 0 ? cursor : composingStart;
            int end = composingEnd < 0 ? cursor : composingEnd;
            int length = Math.min(value.length(), Math.max(0, limit - text.length() + end - start));
            text = text.substring(0, start) + value.substring(0, length) + text.substring(end);
            cursor = start + length;
            boolean marked = compose && length > 0 && !(losesSpan && length < value.length());
            composingStart = marked ? start : -1;
            composingEnd = marked ? cursor : -1;
        }
    }

    private static String syntheticText(int count) {
        return new String(new char[count]).replace('\0', '甲');
    }
    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }
    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters); method.setAccessible(true); return method;
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
