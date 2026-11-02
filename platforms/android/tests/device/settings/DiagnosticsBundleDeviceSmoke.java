package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import org.json.JSONObject;

/** 用安装后的原生宿主生成合成日志包，验证本地事件的写入、隐私过滤和导出，不访问云端。MCP 上传前的重建从 5c 起在 Rust 的 diagnostics_upload 里（#8201 迁移计划），由 client-services 的 diagnostics::upload 测试覆盖。 */
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
            write(perf, "{\"t_ms\":20,\"kind\":\"candidate_shown\",\"duration_ms\":9}\n"
                + "{\"t_ms\":21,\"kind\":\"engine_call\",\"duration_ms\":6}\n"
                + "{\"t_ms\":22,\"kind\":\"candidate_rendered\",\"duration_ms\":14}\n");
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
            check(counts.getJSONObject("performance_logs").getInt("kept") == 3
                && counts.getJSONObject("performance_logs").getInt("dropped") == 0, "keystroke split timings are kept");
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
            stage = "cleanup";
            remove(root);
            root = null;
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: native diagnostics bundle, privacy and local failure retention\n");
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

    private static void write(Path path, String text) throws Exception {
        Files.write(path, text.getBytes(StandardCharsets.UTF_8));
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
