package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.json.JSONObject;

/** 用安装后的原生宿主生成合成日志包，验证上传前的读取和编码，不访问云端。 */
public final class DiagnosticsBundleDeviceSmoke extends Instrumentation {
    private String stage = "native bundle";

    @Override public void onCreate(Bundle arguments) { super.onCreate(arguments); start(); }

    @Override public void onStart() {
        Bundle result = new Bundle();
        Path root = null;
        try {
            root = Files.createTempDirectory(getTargetContext().getCacheDir().toPath(), "diagnostics-test-");
            Path input = root.resolve("input.jsonl");
            Path perf = root.resolve("perf.jsonl");
            Path crash = root.resolve("crash.jsonl");
            write(input, "{\"t_ms\":10,\"kind\":\"key_down\"}\n"
                + "{\"t_ms\":11,\"kind\":\"key_down\",\"text\":\"synthetic-private-marker\"}\n");
            write(perf, "{\"t_ms\":20,\"kind\":\"candidate_shown\",\"duration_ms\":9}\n");
            write(crash, "{\"at\":\"2026-10-14T00:00:00Z\",\"message\":\"java.lang.IllegalStateException\","
                + "\"stack\":\"at synthetic.Example.run(Example.java:7)\"}\n");
            Path archive = root.resolve("native.zip");
            JSONObject include = new JSONObject().put("crash_logs", true).put("performance_logs", true)
                .put("input_events", true).put("config_snapshot", false);
            JSONObject request = new JSONObject().put("state_root", root.toString())
                .put("include", include).put("destination", archive.toString())
                .put("sources", new JSONObject().put("input_events", input.toString())
                    .put("performance_logs", perf.toString()).put("crash_logs", crash.toString()));
            // 从产品加载类，实际追加本地事件，再由 JNI 生成 ZIP。
            ClassLoader loader = getTargetContext().getClassLoader();
            Class<?> eventKind = loader.loadClass("app.msime.android.DiagnosticEventKind");
            Class<?> eventLog = loader.loadClass("app.msime.android.ImeDebugOverlay$EventLog");
            var constructor = eventLog.getDeclaredConstructor(java.io.File.class);
            constructor.setAccessible(true);
            Object log = constructor.newInstance(root.toFile());
            // 事件写入器使用固定文件名，不读产品自己的日志目录。
            Path recorded = root.resolve("input-events.jsonl");
            Files.move(input, recorded);
            var event = eventLog.getDeclaredMethod("event", eventKind);
            event.setAccessible(true);
            String[] localKinds = {"input_failure_runtime_rejected", "input_failure_invalid_response",
                "input_failure_editor_rejected", "input_failure_jni_link",
                "voice_engine_local", "voice_local_model_check", "voice_local_runtime_check",
                "voice_local_recorder_open", "voice_local_native_start", "voice_local_capture_start",
                "voice_local_capture_read_failure", "voice_failure_model", "voice_failure_runtime_missing",
                "voice_failure_permission", "voice_failure_null_pointer"};
            try {
                stage = "real local model refusal";
                Class<?> localRecognizer = loader.loadClass("app.msime.android.LocalAsrRecognizer");
                Object local = localRecognizer.getConstructor().newInstance();
                java.util.List<String> observed = new java.util.ArrayList<>();
                java.util.function.Consumer<Object> sink = kind -> {
                    try {
                        observed.add((String) eventKind.getMethod("wire").invoke(kind));
                        event.invoke(log, kind);
                    } catch (ReflectiveOperationException error) {
                        throw new AssertionError("fixed voice event recording", error);
                    }
                };
                localRecognizer.getMethod("diagnostics", java.util.function.Consumer.class).invoke(local, sink);
                try {
                    localRecognizer.getMethod("recognize", String.class, Path.class, String.class,
                        String.class, String[].class, String[].class,
                        loader.loadClass("app.msime.android.LocalAsrRecognizer$Listener"))
                        .invoke(local, root.resolve("synthetic-missing-model").toString(), root, "zh-CN",
                            "", null, null, null);
                    throw new AssertionError("missing model must refuse");
                } catch (java.lang.reflect.InvocationTargetException expected) {
                    Object failure = expected.getCause();
                    Object kind = failure.getClass().getMethod("diagnosticKind").invoke(failure);
                    check(eventKind.getMethod("wire").invoke(kind).equals("voice_failure_model"),
                        "real model refusal classification");
                    sink.accept(kind);
                }
                check(observed.equals(java.util.Arrays.asList("voice_local_model_check", "voice_failure_model")),
                    "real model refusal stage order");
                for (String kind : localKinds) {
                    if (observed.contains(kind)) continue;
                    Object value = eventKind.getMethod("fromWire", String.class).invoke(null, kind);
                    check(value != null, "installed local failure kind");
                    event.invoke(log, value);
                }
            } finally {
                var shutdown = eventLog.getDeclaredMethod("shutdown");
                shutdown.setAccessible(true);
                shutdown.invoke(log);
                var worker = eventLog.getDeclaredField("worker");
                worker.setAccessible(true);
                check(((java.util.concurrent.ExecutorService) worker.get(log))
                    .awaitTermination(5, java.util.concurrent.TimeUnit.SECONDS), "local writer completed");
            }
            request.getJSONObject("sources").put("input_events", recorded.toString());
            Class<?> nativeClient = loader.loadClass("app.msime.android.NativeClient");
            String nativeResponse = (String) nativeClient.getMethod("diagnosticBundle", String.class)
                .invoke(null, request.toString());
            JSONObject envelope = new JSONObject(nativeResponse);
            check(envelope.getBoolean("ok"), "native bundle generation");
            JSONObject counts = envelope.getJSONObject("value").getJSONObject("counts");
            check(counts.getJSONObject("input_events").getInt("kept") == localKinds.length + 1
                && counts.getJSONObject("input_events").getInt("dropped") == 1, "native privacy filter");
            stage = "local failure ZIP retention";
            try (var zip = new java.util.zip.ZipFile(archive.toFile())) {
                String rows;
                try (var source = zip.getInputStream(zip.getEntry("input_events.ndjson"))) {
                    java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
                    byte[] buffer = new byte[4096];
                    int count;
                    while ((count = source.read(buffer)) != -1) bytes.write(buffer, 0, count);
                    rows = new String(bytes.toByteArray(), StandardCharsets.UTF_8);
                }
                for (String kind : localKinds) check(rows.contains(kind), "local failure survives native export");
                check(!rows.contains("synthetic-private-marker"), "local export rejects private fields");
                check(rows.split("\n").length == localKinds.length + 1, "native export has all fixed events");
            }
            stage = "native bundle upload sections";
            JSONObject body = readBody(loader, archive, true, true, true, false);
            checkSections(body);
            for (String kind : localKinds) check(!body.toString().contains(kind),
                "local failure does not break old cloud contract");
            check(body.getJSONArray("crash_logs").getJSONObject(0).getString("at")
                .equals("2026-10-14T00:00:00Z"), "native crash timestamp retention");
            check(!body.toString().contains("synthetic-private-marker"), "private field exclusion");
            stage = "unselected categories";
            check(readBody(loader, archive, false, false, false, false).length() == 0,
                "unselected categories stay absent");

            stage = "legacy archive aliases";
            Path legacy = root.resolve("legacy.zip");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(legacy))) {
                entry(output, "input-events.jsonl", "{\"t_ms\":10,\"kind\":\"key_down\"}\n");
                entry(output, "perf.jsonl", "{\"t_ms\":20,\"kind\":\"candidate_shown\",\"duration_ms\":9}\n");
                entry(output, "synthetic.crash", "java.lang.IllegalStateException\nat synthetic.Example.run(Example.java:7)");
                entry(output, "config_snapshot.json", "{\"theme\":\"system\"}");
            }
            JSONObject legacyBody = readBody(loader, legacy, true, true, true, true);
            checkSections(legacyBody);
            check(legacyBody.getJSONObject("config_snapshot").getString("theme").equals("system"),
                "configuration remains present");

            stage = "malformed ndjson records";
            Path malformed = root.resolve("malformed.zip");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(malformed))) {
                entry(output, "crash_logs.ndjson", "not-json\n{\"at\":7,\"message\":\"synthetic\",\"stack\":\"\"}\n");
                entry(output, "input_events.ndjson", "{\"t_ms\":10,\"kind\":\"unknown\"}\n");
                entry(output, "performance_logs.ndjson", "{\"t_ms\":20,\"kind\":\"candidate_shown\"}\n");
            }
            JSONObject malformedBody = readBody(loader, malformed, true, true, true, false);
            check(malformedBody.getJSONArray("crash_logs").length() == 0
                && malformedBody.getJSONArray("input_events").length() == 0
                && malformedBody.getJSONArray("perf_trace").length() == 0, "malformed records are dropped");
            stage = "crash record bound";
            Path bounded = root.resolve("bounded.zip");
            String crashLine = "{\"at\":\"2026-10-14T00:00:00Z\",\"message\":\"java.lang.IllegalStateException\",\"stack\":\"\"}\n";
            StringBuilder crashRows = new StringBuilder();
            for (int index = 0; index < 51; index++) crashRows.append(crashLine);
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(bounded))) {
                entry(output, "crash_logs.ndjson", crashRows.toString());
            }
            check(readBody(loader, bounded, true, false, false, false).getJSONArray("crash_logs").length() == 50,
                "crash records stay bounded");
            stage = "cleanup";
            remove(root);
            root = null;
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: native diagnostics bundle, upload sections, privacy, legacy aliases and selection\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: " + stage + " ("
                + error.getClass().getSimpleName() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            if (root != null) {
                try { remove(root); }
                catch (Exception ignored) { /* 合成临时文件由测试进程持有，不读写用户日志。 */ }
            }
        }
    }

    private static JSONObject readBody(ClassLoader loader, Path archive, boolean crash, boolean perf,
            boolean input, boolean config) throws Exception {
        Class<?> api = loader.loadClass("app.msime.android.DiagnosticsApi");
        Class<?> include = loader.loadClass("app.msime.android.DiagnosticsApi$Include");
        Class<?> sections = loader.loadClass("app.msime.android.DiagnosticsApi$Sections");
        Class<?> retention = loader.loadClass("app.msime.android.DiagnosticsApi$Retention");
        Object selection = include.getConstructor(boolean.class, boolean.class, boolean.class, boolean.class)
            .newInstance(crash, perf, input, config);
        Object parsed = api.getMethod("readBundle", java.io.File.class, include)
            .invoke(null, archive.toFile(), selection);
        String body = (String) api.getMethod("requestBody", String.class, String.class, sections, retention)
            .invoke(null, "android", "synthetic", parsed, retention.getField("ONE_DAY").get(null));
        return new JSONObject(body).getJSONObject("sections");
    }

    private static void checkSections(JSONObject body) throws Exception {
        check(body.getJSONArray("input_events").length() == 1, "input event retention");
        check(body.getJSONArray("input_events").getJSONObject(0).getLong("t_ms") == 10, "input event value");
        check(body.getJSONArray("perf_trace").length() == 1, "performance retention");
        check(body.getJSONArray("perf_trace").getJSONObject(0).getLong("duration_ms") == 9, "performance value");
        check(body.getJSONArray("crash_logs").length() == 1, "crash retention");
        check(body.getJSONArray("crash_logs").getJSONObject(0).getString("message")
            .equals("java.lang.IllegalStateException"), "crash value");
    }

    private static void write(Path path, String text) throws Exception {
        Files.write(path, text.getBytes(StandardCharsets.UTF_8));
    }

    private static void entry(ZipOutputStream output, String name, String text) throws Exception {
        output.putNextEntry(new ZipEntry(name));
        output.write(text.getBytes(StandardCharsets.UTF_8));
        output.closeEntry();
    }

    private static void remove(Path root) throws Exception {
        try (var paths = Files.walk(root)) {
            var ordered = paths.sorted(Comparator.reverseOrder()).iterator();
            while (ordered.hasNext()) Files.deleteIfExists(ordered.next());
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
