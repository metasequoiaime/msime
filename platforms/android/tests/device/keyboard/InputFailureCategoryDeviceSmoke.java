package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import android.view.inputmethod.InputConnection;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONException;

/** 从安装产品调用响应解析与编辑器写入，验证失败分类，不读取真实编辑器内容。 */
public final class InputFailureCategoryDeviceSmoke extends Instrumentation {
    private String stage = "create installed service";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Object service = null;
        try {
            ClassLoader loader = getTargetContext().getClassLoader();
            Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
            AtomicReference<Object> created = new AtomicReference<>();
            AtomicReference<Exception> creationError = new AtomicReference<>();
            runOnMainSync(() -> {
                try { created.set(type.getConstructor().newInstance()); }
                catch (Exception error) { creationError.set(error); }
            });
            if (creationError.get() != null) throw creationError.get();
            service = created.get();
            stage = "automatic recovery policy";
            Class<?> recovery = loader.loadClass("app.msime.android.InputFailureRecoveryPolicy");
            Method shouldRetry = method(recovery, "shouldRetry", int.class, boolean.class,
                boolean.class, boolean.class);
            check((boolean) shouldRetry.invoke(null, 0, true, true, true),
                "installed policy retries the first live-session failure");
            check(!(boolean) shouldRetry.invoke(null, 1, true, true, true),
                "installed policy caps recovery at one attempt");
            Method value = method(type, "value", String.class);
            stage = "runtime rejection category";
            checkCategory(loader, failure(value, service,
                "{\"ok\":false,\"error\":\"synthetic-private-marker\"}"), "runtime_rejected");
            stage = "malformed response categories";
            for (String response : new String[] {"not-json", "{}", "{\"ok\":\"true\"}",
                    "{\"ok\":true,\"value\":null}"}) {
                checkCategory(loader, failure(value, service, response), "invalid_response");
            }
            stage = "editor rejection category";
            // 隔离对象只测试写入路径，固定布局提示，避免读取用户的 SharedPreferences。
            Field layout = type.getDeclaredField("touchLayoutHint");
            layout.setAccessible(true);
            layout.setInt(service, 0);
            // apply() 先比较已画键面和传入的 view。真实输入法在 onCreateInputView
            // 里创建这个辅助对象；测试夹具也创建同一个轻量对象，让拒写继续走到
            // EditorBridge，而不是在夹具里先因 NullPointerException 失败。
            Field rows = type.getDeclaredField("imeLetterRows");
            rows.setAccessible(true);
            var rowsConstructor = rows.getType().getDeclaredConstructor(type);
            rowsConstructor.setAccessible(true);
            rows.set(service, rowsConstructor.newInstance(service));
            int[] writes = new int[3];
            InputConnection connection = (InputConnection) Proxy.newProxyInstance(
                InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                (proxy, call, args) -> {
                    return switch (call.getName()) {
                        case "beginBatchEdit" -> { writes[0]++; yield true; }
                        case "setComposingText" -> { writes[1]++; yield false; }
                        case "endBatchEdit" -> { writes[2]++; yield true; }
                        default -> throw new AssertionError("Unexpected editor operation");
                    };
                });
            Field target = type.getDeclaredField("connection");
            target.setAccessible(true);
            target.set(service, connection);
            Method apply = method(type, "apply", String.class);
            Throwable rejected = failure(apply, service, "{\"ok\":true,\"value\":{"
                + "\"view\":{\"scheme\":0,\"editing_text\":\"synthetic\"},\"commit\":null}}");
            stage = "editor rejection category: " + rejected.getClass().getSimpleName();
            checkCategory(loader, rejected, "editor_rejected");
            check(writes[0] == 1 && writes[1] == 1 && writes[2] == 1,
                "rejected editor write remains one ordered attempt");
            stage = "disconnected auxiliary editor operations";
            InputConnection disconnected = (InputConnection) Proxy.newProxyInstance(
                InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                (proxy, call, args) -> {
                    throw new IllegalStateException("synthetic disconnected editor");
                });
            target.set(service, disconnected);
            Field view = type.getDeclaredField("view");
            view.setAccessible(true);
            view.set(service, null);
            for (String operation : new String[] {"deleteBeforeCursor", "deleteCodePointBeforeCursor",
                    "performEditorAction", "stepOverClosingMark", "editorContext", "selectedEditorText"}) {
                stage = "disconnected editor: " + operation;
                switch (operation) {
                    case "deleteBeforeCursor", "performEditorAction" ->
                        method(type, operation, int.class).invoke(service, 1);
                    case "stepOverClosingMark" ->
                        method(type, operation, String.class).invoke(service, ")");
                    case "editorContext" ->
                        check(method(type, operation, boolean.class).invoke(service, true) == null,
                            "disconnected context is unavailable");
                    default -> method(type, operation).invoke(service);
                }
            }
            stage = "editor disconnects after an empty context read";
            int[] keyEvents = new int[1];
            InputConnection emptyThenDisconnected = (InputConnection) Proxy.newProxyInstance(
                InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
                (proxy, call, args) -> switch (call.getName()) {
                    case "getSelectedText" -> null;
                    case "getTextBeforeCursor" -> "";
                    case "sendKeyEvent" -> {
                        keyEvents[0]++;
                        throw new IllegalStateException("synthetic disconnected editor");
                    }
                    default -> throw new AssertionError("Unexpected editor operation");
                });
            target.set(service, emptyThenDisconnected);
            method(type, "deleteCodePointBeforeCursor").invoke(service);
            check(keyEvents[0] == 2, "empty editor sends one guarded down/up pair");
            target.set(service, disconnected);
            stage = "disconnected email context";
            Field inputType = type.getDeclaredField("editorInputType");
            inputType.setAccessible(true);
            inputType.setInt(service, android.text.InputType.TYPE_CLASS_TEXT
                | android.text.InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
            Field email = type.getDeclaredField("imeEmailSuffixes");
            email.setAccessible(true);
            var suffixConstructor = email.getType().getDeclaredConstructor(type);
            suffixConstructor.setAccessible(true);
            Object suffixes = suffixConstructor.newInstance(service);
            method(suffixes.getClass(), "refresh").invoke(suffixes);
            stage = "disconnected edit panel";
            Field panel = type.getDeclaredField("imeTextEditPanel");
            panel.setAccessible(true);
            var panelConstructor = panel.getType().getDeclaredConstructor(type);
            panelConstructor.setAccessible(true);
            Object editPanel = panelConstructor.newInstance(service);
            Class<?> action = loader.loadClass("app.msime.android.TextEditPanelModel$Action");
            for (Object valueAction : action.getEnumConstants()) {
                if (valueAction.toString().equals("PASTE")) continue;
                method(editPanel.getClass(), "perform", action).invoke(editPanel, valueAction);
            }
            stage = "private exception exclusion";
            checkCategory(loader, new JSONException("synthetic-private-marker") {
                private static final long serialVersionUID = 1L;
                @Override public String getMessage() { throw new AssertionError("Exception text read"); }
                @Override public synchronized Throwable getCause() { throw new AssertionError("Exception cause read"); }
            }, "invalid_response");
            checkCategory(loader, new UnsatisfiedLinkError("synthetic-private-marker") {
                private static final long serialVersionUID = 1L;
                @Override public String getMessage() { throw new AssertionError("Linkage text read"); }
            }, "jni_link");
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: installed response and editor failure categories, private exception exclusion\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: " + stage + " ("
                + error.getClass().getSimpleName() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            if (service != null) {
                for (Field field : service.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(field.getType())) continue;
                    try {
                        field.setAccessible(true);
                        ((ExecutorService) field.get(service)).shutdownNow();
                    } catch (ReflectiveOperationException ignored) {
                        // 未启动输入会话，测试对象的工作线程只负责本次隔离检查。
                    }
                }
            }
        }
    }

    private static Method method(Class<?> type, String name, Class<?> parameter) throws Exception {
        Method method = type.getDeclaredMethod(name, parameter);
        method.setAccessible(true);
        return method;
    }

    private static Method method(Class<?> type, String name, Class<?>... parameters)
            throws Exception {
        Method method = type.getDeclaredMethod(name, parameters);
        method.setAccessible(true);
        return method;
    }

    private static Throwable failure(Method method, Object target, String response) throws Exception {
        try { method.invoke(target, response); }
        catch (InvocationTargetException error) { return error.getCause(); }
        throw new AssertionError("Expected input failure");
    }

    private static void checkCategory(ClassLoader loader, Throwable error, String expected) throws Exception {
        final Class<?> policy;
        try { policy = loader.loadClass("app.msime.android.InputFailurePolicy"); }
        catch (ClassNotFoundException missing) { throw new AssertionError("Failure category unavailable"); }
        String message = (String) method(policy, "logMessage", Throwable.class).invoke(null, error);
        check(message.equals("input_failure reason=" + expected), "failure category");
        Object event = method(policy, "eventKind", Throwable.class).invoke(null, error);
        String wire = (String) event.getClass().getMethod("wire").invoke(event);
        check(wire.equals("input_failure_" + expected), "local export retains failure category");
        check(!(boolean) event.getClass().getMethod("uploadable").invoke(event), "local failure is not uploaded");
        check(!message.contains("synthetic-private-marker"), "private exception exclusion");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
