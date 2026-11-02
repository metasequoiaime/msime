package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Bundle;
import android.text.InputFilter;
import android.text.InputType;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.ExtractedText;
import android.view.inputmethod.InputConnection;
import android.widget.EditText;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import org.json.JSONArray;
import org.json.JSONObject;

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
                for (String suffix : new String[] {"\"", "🌲e\u0301", "אב"}) {
                    stage = "quick phrase cursor and deferred composition: " + suffix.length();
                    String commit = "title:" + suffix;
                    editor = fixture.reset("甲乙丙丁后文", 100, false);
                    editor.cursor = editor.selectionEnd = 4;
                    editor.windowStart = 2;
                    fixture.resetSelection(4, 4);
                    check(fixture.apply(commit, "n", suffix), "cursor and next composition accepted");
                    check(editor.text.equals("甲乙丙丁title:n" + suffix + "后文"), "new composition precedes entire suffix");
                    check(editor.cursor == 11 && editor.composingStart == 10, "UTF-16 destination and composition");
                    check(editor.readsInBatch == 0 && editor.depth == 0, "cursor read follows flushed commit");
                    fixture.acknowledge(editor);
                }
                stage = "quick phrase replaces selection";
                editor = fixture.reset("甲替换乙", 100, false);
                editor.cursor = 1;
                editor.selectionEnd = 3;
                fixture.resetSelection(1, 3);
                check(fixture.apply("[]", "", "]"), "selection replaced");
                check(editor.text.equals("甲[]乙") && editor.cursor == 2, "position belongs to replacement");
                fixture.acknowledge(editor);
                stage = "quick phrase replaces composition";
                editor = fixture.reset("甲", 100, false);
                check(fixture.apply(null, "ni"), "composition starts");
                check(fixture.apply("[]", "", "]"), "composition replaced");
                check(editor.text.equals("甲[]") && editor.cursor == 2, "position belongs to committed phrase");
                fixture.acknowledge(editor);
                for (int fallback = 0; fallback < 6; fallback++) {
                    stage = "quick phrase safe fallback: " + fallback;
                    editor = fixture.reset("甲", fallback == 0 ? 2 : 100, false);
                    editor.unknown = fallback == 1;
                    editor.rejectSelection = fallback == 2;
                    editor.disconnectSelection = fallback == 3;
                    editor.changedExtraction = fallback == 4;
                    editor.partialExtraction = fallback == 5;
                    check(fixture.apply("[]", "", "]"), "failed positioning keeps successful commit");
                    check(editor.text.equals(fallback == 0 ? "甲[" : "甲[]"), "fallback never rewrites committed text");
                    check(editor.cursor == editor.text.length(), "fallback retains editor cursor");
                    check(editor.selectionWrites == (fallback == 2 || fallback == 3 ? 1 : 0),
                        "unverified text must never move selection");
                }
                stage = "native EditText quick phrase cursor";
                EditText nativeEditor = new EditText(getTargetContext());
                nativeEditor.setInputType(InputType.TYPE_CLASS_TEXT);
                nativeEditor.setText("甲替换乙");
                nativeEditor.setSelection(1, 3);
                fixture.resetConnection(nativeEditor.onCreateInputConnection(new EditorInfo()), 1);
                fixture.resetSelection(1, 3);
                check(fixture.apply("title:🌲e\u0301", "", "🌲e\u0301"), "native Unicode phrase accepted");
                check(nativeEditor.getText().toString().equals("甲title:🌲e\u0301乙"), "native replacement retains surrounding text");
                check(nativeEditor.getSelectionStart() == 7 && nativeEditor.getSelectionEnd() == 7,
                    "native cursor precedes emoji and combining sequence");
                nativeEditor.setFilters(new InputFilter[] {new InputFilter.LengthFilter(nativeEditor.length() + 1)});
                nativeEditor.setSelection(nativeEditor.length());
                fixture.resetConnection(nativeEditor.onCreateInputConnection(new EditorInfo()), nativeEditor.length());
                int originalLength = nativeEditor.length();
                check(fixture.apply("[]", "", "]"), "native truncated commit remains accepted");
                check(nativeEditor.length() == originalLength + 1 && nativeEditor.getSelectionStart() == nativeEditor.length(),
                    "native truncated commit must not position inside earlier text");
                stage = "cleanup rejection";
                editor = fixture.reset(syntheticText(30), 30, true);
                editor.rejectFinish = true;
                check(!fixture.apply(null, "n") && editor.depth == 0, "cleanup failure reaches caller");
                stage = "composition rejection";
                editor = fixture.reset("", 100, false);
                editor.rejectCompose = true;
                check(!fixture.apply(null, "n") && editor.reads == 0 && editor.depth == 0,
                    "failed write skips verification");
                stage = "installed Engine phrase through host apply into native editor";
                runtimePhrases(fixture);
            } catch (Exception | AssertionError error) {
                failure[0] = error;
            } finally {
                if (service != null) {
                    try { service.getClass().getMethod("onDestroy").invoke(service); }
                    catch (ReflectiveOperationException ignored) { /* 清理隔离服务。 */ }
                }
                if (service != null) for (Field member : service.getClass().getDeclaredFields()) {
                    if (!ExecutorService.class.isAssignableFrom(member.getType())) continue;
                    try { member.setAccessible(true); ((ExecutorService) member.get(service)).shutdownNow(); }
                    catch (ReflectiveOperationException ignored) { /* 隔离服务未启动真实输入会话。 */ }
                }
            }
        });
        boolean passed = failure[0] == null;
        result.putString("stream", passed
            ? "MSIME_DEVICE_SMOKE_PASSED: installed composition batch flush, phrase cursor, native editor and selection echoes\n"
            : "MSIME_DEVICE_SMOKE_FAILED: " + stage + " (" + failure[0].getClass().getSimpleName() + ")"
                + (failure[0] instanceof AssertionError ? ": " + failure[0].getMessage() : "") + "\n");
        finish(passed ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
    }

    /** 临时短语表和写目录完全隔离，使用已安装的 Rust 引擎与宿主解析真实响应。 */
    private void runtimePhrases(Fixture fixture) throws Exception {
        File root = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "phrase-cursor-").toFile();
        Class<?> nativeType = getTargetContext().getClassLoader().loadClass("app.msime.android.NativeClient");
        long session = 0;
        try {
            String phrase = "title:\"${cursor}🌲e\u0301\"";
            String expanded = "title:\"🌲e\u0301\"";
            File pack = new File(root, "state/plugins/phrase_table/cursor");
            check(pack.mkdirs(), "temporary phrase directory");
            String manifest = "schema_version = 1\nkind = \"phrase_table\"\nid = \"cursor\"\n"
                + "name = \"cursor\"\nversion = \"1\"\nlicense = \"CC0-1.0\"\n"
                + "[[phrases]]\nkey = \"t\"\ntext = " + JSONObject.quote(phrase) + "\n";
            Files.write(new File(pack, "plugin.toml").toPath(), manifest.getBytes(StandardCharsets.UTF_8));
            JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(
                getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
            for (String name : new String[] {"user_data", "cache", "preferences_directory"}) {
                File directory = new File(root, name.equals("preferences_directory") ? "state" : name);
                check(directory.isDirectory() || directory.mkdirs(), "temporary runtime directory");
                options.put(name, directory.getAbsolutePath());
            }
            // 混输只在隔离会话声明能力以验证响应接线；Android 产品的既有能力声明和默认值不变。
            options.put("selection_statistics", false).put("quick_phrase_candidates", true);
            JSONObject preferences = options.getJSONObject("preferences");
            preferences.put("scheme", "quanpin").put("default_ime_mode", "chinese")
                .put("learning", false).put("cloud_candidates", false).put("chinese_english_spacing", false);
            nested(preferences, "plugins").put("phrase_tables", new JSONArray().put("cursor"));
            nested(preferences, "local_modes").put("quick_phrase", true);
            nested(preferences, "mixed_input").put("quick_phrase", true);
            stage = "initialize installed service for phrase cursor";
            fixture.type.getMethod("onCreate").invoke(fixture.service);
            fixture.type.getMethod("onCreateInputView").invoke(fixture.service);
            field(fixture.type, "renderingSuppressed").setBoolean(fixture.service, true);
            field(fixture.type, "touchLayoutHint").setInt(fixture.service, 0);
            Method applyResponse = method(fixture.type, "apply", String.class);
            Method character = nativeType.getMethod("character", long.class, int.class, boolean.class);
            for (boolean kMode : new boolean[] {true, false}) {
                stage = "create native phrase session, K mode=" + kMode;
                JSONObject initial = value(nativeType.getMethod("create", String.class).invoke(null, options.toString()));
                session = initial.getLong("session");
                field(fixture.type, "session").setLong(fixture.service, session);
                value(nativeType.getMethod("focus", long.class, boolean.class).invoke(null, session, true));
                field(fixture.type, "view").set(fixture.service, value(nativeType.getMethod("setEnglishMode",
                    long.class, boolean.class).invoke(null, session, false)));
                EditText editor = new EditText(getTargetContext());
                editor.setInputType(InputType.TYPE_CLASS_TEXT);
                editor.setText("甲乙");
                editor.setSelection(1);
                fixture.resetConnection(editor.onCreateInputConnection(new EditorInfo()), 1);
                stage = "type native phrase code, K mode=" + kMode;
                for (char key : (kMode ? "Kt" : "t").toCharArray()) {
                    String response = (String) character.invoke(null, session, (int) key, Character.isUpperCase(key));
                    applyResponse.invoke(fixture.service, response);
                }
                JSONObject view = (JSONObject) field(fixture.type, "view").get(fixture.service);
                stage = "find native phrase preview, K mode=" + kMode;
                JSONArray candidates = view.getJSONArray("candidates");
                JSONObject identity = null;
                for (int index = 0; index < candidates.length(); index++) {
                    JSONObject candidate = candidates.getJSONObject(index);
                    if (expanded.equals(candidate.getString("text"))) identity = candidate.getJSONObject("id");
                }
                check(identity != null, "expanded phrase candidate, K mode=" + kMode);
                stage = "commit native phrase, K mode=" + kMode;
                String response = (String) nativeType.getMethod("select", long.class, long.class, long.class)
                    .invoke(null, session, identity.getLong("generation"), identity.getLong("index"));
                JSONObject committed = value(response);
                check(committed.opt("commit_cursor_suffix") instanceof String, "runtime suffix exists after JNI");
                check(committed.getString("commit_cursor_suffix").equals("🌲e\u0301\""), "runtime suffix survives JNI");
                stage = "apply native phrase to editor, K mode=" + kMode;
                applyResponse.invoke(fixture.service, response);
                check(editor.getText().toString().equals("甲" + expanded + "乙"), "native host commits expanded phrase exactly once");
                check(editor.getSelectionStart() == 8 && editor.getSelectionEnd() == 8, "host consumes real runtime cursor");
                check(fixture.apply("合成", "") && editor.getText().toString()
                    .equals("甲title:\"合成🌲e\u0301\"乙"), "typing continues at requested position");
                stage = "destroy native phrase session, K mode=" + kMode;
                check(new JSONObject((String) nativeType.getMethod("destroy", long.class).invoke(null, session))
                    .getBoolean("ok"), "native session destroyed");
                session = 0;
                field(fixture.type, "session").setLong(fixture.service, 0);
            }
        } finally {
            if (session != 0) nativeType.getMethod("destroy", long.class).invoke(null, session);
            field(fixture.type, "session").setLong(fixture.service, 0);
            removeTemporary(root);
        }
    }

    private static JSONObject value(Object response) throws Exception {
        JSONObject envelope = new JSONObject((String) response);
        check(Boolean.TRUE.equals(envelope.opt("ok")), "native request accepted");
        return envelope.getJSONObject("value");
    }
    private static JSONObject nested(JSONObject parent, String key) throws Exception {
        JSONObject value = parent.optJSONObject(key);
        if (value == null) { value = new JSONObject(); parent.put(key, value); }
        return value;
    }
    private static void removeTemporary(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) removeTemporary(child);
        file.delete();
    }

    private static final class Fixture {
        final Class<?> type;
        final Object service;
        final Object bridge;
        final Object tracker;
        final Object source;
        final Method sink;
        final Method apply;
        final Method applyCursor;
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
            applyCursor = method(bridge.getClass(), "apply", loader.loadClass("app.msime.android.EditorBridge$Sink"),
                String.class, String.class, String.class);
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
        boolean apply(String commit, String composing, String suffix) throws Exception {
            return (boolean) applyCursor.invoke(bridge, sink.invoke(service, source), commit, composing, suffix);
        }
        void resetSelection(int start, int end) throws Exception {
            method(tracker.getClass(), "reset", int.class, int.class).invoke(tracker, start, end);
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
        int selectionEnd;
        int windowStart;
        int selectionWrites;
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
        boolean rejectSelection;
        boolean disconnectSelection;
        boolean changedExtraction;
        boolean partialExtraction;
        final List<Runnable> pending = new ArrayList<>();
        final List<int[]> echoes = new ArrayList<>();
        BufferedEditor(String text, int limit, boolean losesSpan) {
            this.text = text;
            cursor = text.length();
            selectionEnd = cursor;
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
                        enqueue(() -> {
                            text = text.substring(0, cursor - length) + text.substring(cursor);
                            cursor -= length;
                            selectionEnd = cursor;
                        });
                        yield true;
                    }
                    case "getTextBeforeCursor" -> {
                        reads++;
                        if (depth > 0) readsInBatch++;
                        yield unknown ? null : text.substring(Math.max(0, cursor - (int) args[0]), cursor);
                    }
                    case "getExtractedText" -> {
                        reads++;
                        if (depth > 0) readsInBatch++;
                        if (unknown) yield null;
                        ExtractedText extracted = new ExtractedText();
                        extracted.partialStartOffset = partialExtraction ? 0 : -1;
                        extracted.startOffset = windowStart;
                        extracted.text = changedExtraction ? syntheticText(text.length() - windowStart) : text.substring(windowStart);
                        extracted.selectionStart = cursor - windowStart;
                        extracted.selectionEnd = selectionEnd - windowStart;
                        yield extracted;
                    }
                    case "setSelection" -> {
                        selectionWrites++;
                        if (disconnectSelection) throw new IllegalStateException("synthetic disconnect");
                        if (rejectSelection) yield false;
                        cursor = (int) args[0];
                        selectionEnd = (int) args[1];
                        echoes.add(new int[] {cursor, composingStart, composingEnd});
                        yield true;
                    }
                    default -> throw new AssertionError("Unexpected editor operation: " + call.getName());
                });
        }
        void enqueue(Runnable operation) { if (depth == 0) operation.run(); else pending.add(operation); }
        void replace(String value, boolean compose) {
            int start = composingStart < 0 ? Math.min(cursor, selectionEnd) : composingStart;
            int end = composingEnd < 0 ? Math.max(cursor, selectionEnd) : composingEnd;
            int length = Math.min(value.length(), Math.max(0, limit - text.length() + end - start));
            text = text.substring(0, start) + value.substring(0, length) + text.substring(end);
            cursor = start + length;
            selectionEnd = cursor;
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
