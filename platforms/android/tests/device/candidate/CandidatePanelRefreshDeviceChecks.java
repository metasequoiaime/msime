package app.msime.android.test;

import android.app.Instrumentation;
import android.content.Context;
import android.os.Handler;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.InputConnection;
import android.widget.Button;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.ExecutorService;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Supplier;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 在安装产品的真实会话中推进候选代次，覆盖停顿重排和在线回复两个宿主入口。
 *
 * <p>在线回复在面板开着时先等着：面板不换代，提示露出来；点提示（刷新）或收起面板之后才用上。停顿重排的发布入口直接调用，面板开着时不起跑由 PauseRerankControllerSmoke 覆盖。
 */
final class CandidatePanelRefreshDeviceChecks {
    private final Instrumentation instrumentation;
    private final Consumer<String> stage;
    private final Class<?> type;
    private final Class<?> nativeType;
    private Object service;
    private long session;
    private int editorWrites;

    private CandidatePanelRefreshDeviceChecks(Instrumentation instrumentation,
            Consumer<String> stage) throws Exception {
        this.instrumentation = instrumentation;
        this.stage = stage;
        ClassLoader loader = instrumentation.getTargetContext().getClassLoader();
        type = loader.loadClass("app.msime.android.MSIMEInputService");
        nativeType = loader.loadClass("app.msime.android.NativeClient");
    }

    static void run(Instrumentation instrumentation, Consumer<String> stage) throws Exception {
        CandidatePanelRefreshDeviceChecks test = new CandidatePanelRefreshDeviceChecks(instrumentation, stage);
        try {
            test.onMain(test::prepare);
            for (Mode mode : Mode.values()) test.refresh(mode);
            test.onMain(test::typingInvalidatesPanel);
        } finally {
            test.onMain(test::close);
        }
    }

    /** 停顿发布；在线回复点提示刷新；在线回复收起面板。 */
    private enum Mode { PAUSE, ONLINE_REFRESH, ONLINE_CLOSE }

    private void prepare() throws Exception {
        stage.accept("prepare synthetic async candidate refresh");
        Context context = instrumentation.getTargetContext();
        service = type.getConstructor().newInstance();
        Method attach = android.content.ContextWrapper.class.getDeclaredMethod("attachBaseContext", Context.class);
        attach.setAccessible(true);
        attach.invoke(service, context);
        type.getMethod("onCreate").invoke(service);
        type.getMethod("onCreateInputView").invoke(service);
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
            context.getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        options.getJSONObject("preferences").put("scheme", "quanpin").put("learning", false)
            .put("candidate_page_size", 5).put("cloud_candidates", true)
            .put("ai_assistant", new JSONObject().put("enabled", false));
        options.put("phrase_preedit", true);
        session = value(call("create", new Class<?>[] {String.class}, options.toString())).getLong("session");
        value(call("focus", new Class<?>[] {long.class, boolean.class}, session, true));
        field(type, "session").setLong(service, session);
        field(type, "touchLayoutHint").setInt(service, 0);
        field(type, "candidateEnglishGloss").setBoolean(service, false);
        field(type, "renderingSuppressed").setBoolean(service, true);
        // 只向共享内核注入合成回包；即使重画安排了联网任务，也不允许发起真实传输。
        ((ExecutorService) field(type, "onlineCandidateWorker").get(service)).shutdownNow();
        InputConnection connection = (InputConnection) Proxy.newProxyInstance(
            InputConnection.class.getClassLoader(), new Class<?>[] {InputConnection.class},
            (proxy, operation, arguments) -> {
                if (operation.getName().equals("commitText") || operation.getName().equals("setComposingText"))
                    editorWrites++;
                if (operation.getReturnType() == boolean.class) return true;
                if (operation.getReturnType() == int.class) return 0;
                return null;
            });
        field(type, "connection").set(service, connection);
    }

    private void refresh(Mode mode) throws Exception {
        boolean online = mode != Mode.PAUSE;
        JSONObject[] oldId = new JSONObject[1];
        Button[] oldButton = new Button[1];
        int[] writes = new int[1];
        String[] query = new String[1];
        onMain(() -> {
            stage.accept(mode + " refresh keeps expanded candidate panel");
            field(type, "renderingSuppressed").setBoolean(service, true);
            method(type, "closeCandidatePanel").invoke(service);
            method(type, "apply", String.class).invoke(service,
                call("command", new Class<?>[] {long.class, int.class}, session, 3));
            for (char letter : "nihao".toCharArray()) method(type, "apply", String.class).invoke(service,
                call("character", new Class<?>[] {long.class, int.class, boolean.class}, session, (int) letter, false));
            method(type, "openCandidatePanel").invoke(service);
            field(type, "renderingSuppressed").setBoolean(service, false);
            method(type, "render").invoke(service);
            check(field(type, "candidatePanelOpen").getBoolean(service), "panel opened");
            JSONObject snapshot = (JSONObject) field(type, "candidatePanelSnapshot").get(service);
            oldId[0] = snapshot.getJSONArray("candidates").getJSONObject(0).getJSONObject("id");
            oldButton[0] = candidateButton((View) field(type, "expandedCandidates").get(service));
            check(oldButton[0] != null, "expanded candidate button exists");
            query[0] = value(call("onlineQuery", new Class<?>[] {long.class}, session)).toString();
            cancelScheduledWork();
            writes[0] = editorWrites;
        });
        Supplier<String> result = () -> {
            try {
                return (String) call("applyOnlineCandidates",
                    new Class<?>[] {long.class, String.class, String.class, int.class}, session, query[0],
                    new JSONArray().put(online ? "你好呀" : "你好啊").toString(), 0);
            } catch (Exception error) { throw new IllegalStateException(error); }
        };
        if (online) {
            String[] refreshed = new String[1];
            Class<?> requestType = loader().loadClass("app.msime.android.MSIMEInputService$OnlineRequest");
            java.lang.reflect.Constructor<?> create = requestType.getDeclaredConstructor(
                long.class, String.class, long.class, String.class);
            create.setAccessible(true);
            Object request = create.newInstance(session, query[0], field(type, "onlineEpoch").getLong(service), "");
            Consumer<String> then = value -> refreshed[0] = value;
            method(type, "applyOnlineResult", requestType, Integer.class, Supplier.class, Consumer.class)
                .invoke(service, request, 2, result, then);
            onMain(() -> {
                check(field(type, "candidatePanelOpen").getBoolean(service), "held online rows keep panel open");
                JSONObject snapshot = (JSONObject) field(type, "candidatePanelSnapshot").get(service);
                check(snapshot.getLong("generation") == oldId[0].getLong("generation"),
                    "held online rows do not advance the panel");
                check(method(type, "heldPanelSources").invoke(service).equals(List.of(2)), "cloud rows are held");
                View cue = cueView();
                check(cue.getVisibility() == View.VISIBLE, "held rows show the cue");
                check(cue.getContentDescription().toString().contains("云候选"), "cue names the held source");
                if (mode == Mode.ONLINE_REFRESH) check(cue.performClick(), "cue refreshes the panel");
                else {
                    method(type, "closeCandidatePanel").invoke(service);
                    method(type, "render").invoke(service);
                }
            });
            onMain(() -> {
                check(refreshed[0] != null, "online response accepted after release");
                check(cueView().getVisibility() != View.VISIBLE, "cue hides once the rows are used");
            });
            if (mode == Mode.ONLINE_CLOSE) {
                onMain(() -> {
                    cancelScheduledWork();
                    check(!field(type, "candidatePanelOpen").getBoolean(service), "closed panel stays closed");
                    JSONObject current = (JSONObject) field(type, "view").get(service);
                    check(current.getLong("generation") > oldId[0].getLong("generation"),
                        "rows held by the panel apply when it closes");
                });
                return;
            }
        } else {
            onMain(() -> {
                JSONObject reply = value(result.get());
                check(reply.getBoolean("applied"), "synthetic candidate update applied");
                Object pause = field(type, "pauseRerank").get(service);
                Object publish = field(pause.getClass(), "publish").get(pause);
                method(publish.getClass(), "accept", Object.class).invoke(publish, reply.getJSONObject("view"));
            });
        }
        onMain(() -> {
            cancelScheduledWork();
            check(field(type, "candidatePanelOpen").getBoolean(service), "async update kept panel open");
            check(((View) field(type, "expandedCandidates").get(service)).getVisibility() == View.VISIBLE,
                "expanded grid remains visible");
            JSONObject snapshot = (JSONObject) field(type, "candidatePanelSnapshot").get(service);
            JSONObject actual = value(call("allCandidates", new Class<?>[] {long.class}, session));
            check(snapshot.getLong("generation") > oldId[0].getLong("generation"), "generation advanced");
            check(snapshot.getJSONArray("candidates").toString().equals(actual.getJSONArray("candidates").toString()),
                "fresh candidate identities and order match runtime");
            oldButton[0].performClick();
            check(editorWrites == writes[0], "refresh and stale callback do not write editor");
            check(field(type, "session").getLong(service) == session, "stale callback preserves session");
            method(type, "closeCandidatePanel").invoke(service);
            JSONObject current = (JSONObject) field(type, "view").get(service);
            Object pause = field(type, "pauseRerank").get(service);
            Object publish = field(pause.getClass(), "publish").get(pause);
            method(publish.getClass(), "accept", Object.class).invoke(publish, current);
            check(!field(type, "candidatePanelOpen").getBoolean(service), "refresh does not reopen closed panel");
            cancelScheduledWork();
        });
    }

    private ClassLoader loader() {
        return instrumentation.getTargetContext().getClassLoader();
    }

    private View cueView() throws Exception {
        Object cue = field(type, "imeCandidatePanelCue").get(service);
        return (View) field(cue.getClass(), "pill").get(cue);
    }

    private void cancelScheduledWork() throws Exception {
        Handler main = (Handler) field(type, "main").get(service);
        main.removeCallbacksAndMessages(null);
        field(type, "onlineTask").set(service, null);
    }

    private void typingInvalidatesPanel() throws Exception {
        stage.accept("typing still invalidates old expanded snapshot");
        method(type, "openCandidatePanel", boolean.class).invoke(service, true);
        check(field(type, "candidatePanelOpen").getBoolean(service), "panel opened before typing");
        JSONObject reply = value(call("character", new Class<?>[] {long.class, int.class, boolean.class},
            session, (int) 'a', false));
        Object pause = field(type, "pauseRerank").get(service);
        Object publish = field(pause.getClass(), "publish").get(pause);
        method(publish.getClass(), "accept", Object.class).invoke(publish, reply.getJSONObject("view"));
        check(!field(type, "candidatePanelOpen").getBoolean(service), "different composition closes old panel");
        cancelScheduledWork();
    }

    private void close() throws Exception {
        if (service == null) return;
        cancelScheduledWork();
        if (session != 0) {
            call("destroy", new Class<?>[] {long.class}, session);
            field(type, "session").setLong(service, 0);
        }
        type.getMethod("onDestroy").invoke(service);
        for (Field member : type.getDeclaredFields()) {
            if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
            member.setAccessible(true);
            ((ExecutorService) member.get(service)).shutdownNow();
        }
    }

    private Object call(String name, Class<?>[] parameters, Object... arguments) throws Exception {
        return nativeType.getMethod(name, parameters).invoke(null, arguments);
    }
    private static Button candidateButton(View view) {
        if (view instanceof Button button && button.getContentDescription() != null
                && button.getContentDescription().toString().startsWith("候选 ")) return button;
        if (view instanceof ViewGroup group) for (int index = 0; index < group.getChildCount(); index++) {
            Button found = candidateButton(group.getChildAt(index));
            if (found != null) return found;
        }
        return null;
    }
    @FunctionalInterface private interface Action { void run() throws Exception; }
    private void onMain(Action action) throws Exception {
        Throwable[] failure = new Throwable[1];
        instrumentation.runOnMainSync(() -> {
            try { action.run(); } catch (Exception | AssertionError error) { failure[0] = error; }
        });
        if (failure[0] instanceof Exception error) throw error;
        if (failure[0] instanceof AssertionError error) throw error;
    }
    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name); field.setAccessible(true); return field;
    }
    private static Method method(Class<?> type, String name, Class<?>... parameters) throws Exception {
        Method method = type.getDeclaredMethod(name, parameters); method.setAccessible(true); return method;
    }
    private JSONObject value(Object response) throws Exception {
        JSONObject result = new JSONObject((String) response);
        check(result.getBoolean("ok"), "native operation accepted");
        return result.getJSONObject("value");
    }
    private void check(boolean condition, String description) {
        if (!condition) {
            stage.accept(description);
            throw new AssertionError(description);
        }
    }
}
