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
import org.json.JSONException;

/** 从安装产品调用响应解析与编辑器写入，验证失败分类，不读取真实编辑器内容。 */
public final class InputFailureCategoryDeviceSmoke extends Instrumentation {
    private String stage = "create installed service";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        // 整个测试体在主线程上跑，和 ToolbarEdgesDeviceSmoke 等设备冒烟一致：服务里的协作对象和键区
        // 只在 onCreate / onCreateInputView 里建出来，而 render() 会改真实按钮的 selected / activated，
        // 那会驱动主题里的 StateListAnimator，在 instrumentation 线程上抛「Animators may only be run on
        // Looper threads」。文本编辑面板的「数字键盘」动作正是从 render() 进来的（#8496 让它进了本套件
        // 的动作遍历）。
        Throwable[] failure = new Throwable[1];
        runOnMainSync(() -> {
            try {
                runStages();
            } catch (Exception | AssertionError error) {
                failure[0] = error;
            }
        });
        Bundle result = new Bundle();
        Throwable error = failure[0];
        result.putString("stream", error == null
            ? "MSIME_DEVICE_SMOKE_PASSED: installed response and editor failure categories, private exception exclusion\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " ("
                + error.getClass().getSimpleName() + ")" + causeChain(error) + "\n");
        finish(error == null ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    /** 各个阶段本身；必须在主线程上跑的原因见 {@link #onStart()} 的注释。 */
    private void runStages() throws Exception {
        Object service = null;
        try {
            ClassLoader loader = getTargetContext().getClassLoader();
            Class<?> type = loader.loadClass("app.msime.android.MSIMEInputService");
            service = type.getConstructor().newInstance();
            Method attach = android.content.ContextWrapper.class.getDeclaredMethod(
                "attachBaseContext", android.content.Context.class);
            attach.setAccessible(true);
            attach.invoke(service, getTargetContext());
            type.getMethod("onCreate").invoke(service);
            type.getMethod("onCreateInputView").invoke(service);
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
            // 固定布局提示，让拒写这一路按确定的键面比较。
            Field layout = type.getDeclaredField("touchLayoutHint");
            layout.setAccessible(true);
            layout.setInt(service, 0);
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
                // 粘贴读取系统剪贴板，数字键切换完整键面；两者不属于隔离编辑器连接检查。
                if (valueAction.toString().equals("PASTE") || valueAction.toString().equals("NUMBER_PAD")) continue;
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
        } finally {
            if (service != null) {
                try {
                    service.getClass().getMethod("onDestroy").invoke(service);
                } catch (ReflectiveOperationException ignored) {
                    // onDestroy 自己负责注销监听、取消回调和关闭服务组件；反射失败时继续清理线程。
                }
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

    /**
     * 失败的根因：反射调用抛的是 {@code InvocationTargetException}，只报包装类型定位不到真正的抛出点，
     * 所以要顺着 cause 一路打出来，并各带几帧栈。断言失败没有 cause，那就是它自己。
     */
    private static String causeChain(Throwable error) {
        StringBuilder text = new StringBuilder();
        for (Throwable cause = error; cause != null; cause = cause.getCause()) {
            text.append("\n  ").append(cause.getClass().getName()).append(": ").append(cause.getMessage());
            StackTraceElement[] frames = cause.getStackTrace();
            for (int index = 0; index < Math.min(6, frames.length); index++)
                text.append("\n    at ").append(frames[index]);
        }
        return text.toString();
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
