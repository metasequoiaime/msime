package app.msime.android.test;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.PosixFilePermissions;
import java.util.Arrays;
import java.util.UUID;
import org.json.JSONObject;

/** 使用产品 JNI 检查真实应用 UID 下的共享授权；只读沙箱以真实权限模拟，不访问用户偏好或网络。 */
final class ReportingAuthorityDeviceChecks {
    private ReportingAuthorityDeviceChecks() {}

    private static void require(boolean value, String label) {
        if (!value) throw new AssertionError(label);
    }

    private static JSONObject call(Class<?> client, String method, JSONObject request) throws Exception {
        try {
            return new JSONObject((String) client.getMethod(method, String.class).invoke(null, request.toString()));
        } catch (java.lang.reflect.InvocationTargetException failure) {
            Throwable cause = failure.getCause();
            throw new AssertionError("product JNI failed: " + (cause == null
                ? failure.getClass().getSimpleName() : cause.getClass().getSimpleName()));
        }
    }

    private static JSONObject value(JSONObject reply) throws Exception {
        require(reply.getBoolean("ok"), "product JNI refused synthetic request");
        return reply.getJSONObject("value");
    }

    private static JSONObject query(Path authority) throws Exception {
        return new JSONObject().put("authority_directory", authority.toString());
    }

    private static JSONObject publish(Class<?> client, Path source, Path authority, boolean allowed) throws Exception {
        return value(call(client, "telemetryConsent", query(authority)
            .put("directory", source.toString()).put("publication", new JSONObject()
                .put("policy_allowed", allowed).put("policy_identity", "synthetic-device-policy"))));
    }

    private static void writable(Path authority, Path lock, Path document) throws Exception {
        Files.setPosixFilePermissions(authority, PosixFilePermissions.fromString("rwx------"));
        Files.setPosixFilePermissions(lock, PosixFilePermissions.fromString("rw-------"));
        Files.setPosixFilePermissions(document, PosixFilePermissions.fromString("rw-------"));
    }

    static void run(Path root, ClassLoader loader, String expectedRevision) throws Exception {
        require(android.os.Process.myUid() >= 10000, "requires real application UID, not root or shell");
        require(expectedRevision.matches("[0-9a-f]{40}"), "requires exact expected product revision");
        Class<?> client = loader.loadClass("app.msime.android.NativeClient");
        Path source = root.resolve("source");
        Path authority = root.resolve("authority");
        Path queue = root.resolve("queue");
        Path absent = root.resolve("absent");
        require(!call(client, "telemetryConsent", query(absent)).getBoolean("ok"), "missing authority must refuse");
        require(!Files.exists(absent), "reader must not create a directory or lock");
        JSONObject first = publish(client, source, authority, true);
        require(first.getBoolean("enabled"), "publisher grants synthetic consent");
        String epoch = first.getString("epoch");
        require(UUID.fromString(epoch).toString().equals(epoch), "canonical Rust consent identity");
        Path lock = authority.resolve("authority.lock");
        Path document = authority.resolve("authority.json");
        byte[] original = Files.readAllBytes(document);
        JSONObject capture;
        JSONObject begin = new JSONObject().put("directory", queue.toString())
            .put("authority_directory", authority.toString()).put("platform", "android")
            .put("version", "synthetic-authority").put("component", "authority-device-test")
            .put("capture_only", true).put("consent_epoch", epoch);
        try {
            Files.setPosixFilePermissions(lock, PosixFilePermissions.fromString("r--------"));
            Files.setPosixFilePermissions(document, PosixFilePermissions.fromString("r--------"));
            Files.setPosixFilePermissions(authority, PosixFilePermissions.fromString("r-x------"));
            require(!Files.isWritable(authority) && !Files.isWritable(lock) && !Files.isWritable(document),
                "application UID must actually lack write access");
            require(value(call(client, "telemetryConsent", query(authority))).getString("epoch").equals(epoch),
                "read-only lock and document retain current authority");
            capture = value(call(client, "telemetryBegin", begin));
            require(capture.getBoolean("enabled"), "read-only authority permits private capture preparation");
            String header = capture.getString("crash_record_header");
            require(header.startsWith("MSIME-CRASH/1 "), "product capture header format");
            JSONObject identity = new JSONObject(header.substring("MSIME-CRASH/1 ".length()));
            require(identity.getString("revision").equals(expectedRevision), "exact frozen product revision");
            require(identity.getString("component").equals("authority-device-test"), "original capture component");
            require(Arrays.equals(original, Files.readAllBytes(document)), "reader must not rewrite authority");
            try (java.util.stream.Stream<Path> entries = Files.list(authority)) {
                require(entries.count() == 2, "reader must not create temporary files");
            }
        } finally { writable(authority, lock, document); }

        Files.write(document, "{".getBytes(StandardCharsets.US_ASCII));
        require(!call(client, "telemetryConsent", query(authority)).getBoolean("ok"), "corruption must refuse");
        Files.write(document, original);
        require(value(call(client, "telemetryConsent", query(authority))).getString("epoch").equals(epoch),
            "same-source recovery retains authority");
        Path saved = authority.resolve("saved-document");
        Files.move(document, saved);
        require(!call(client, "telemetryConsent", query(authority)).getBoolean("ok"), "missing document must refuse");
        require(!Files.exists(document), "reader must not initialize missing authority");
        Files.move(saved, document);
        Path alias = root.resolve("linked-authority");
        Files.createSymbolicLink(alias, authority);
        require(!call(client, "telemetryConsent", query(alias)).getBoolean("ok"), "symbolic authority must refuse");
        require(!publish(client, source, authority, false).getBoolean("enabled"), "policy withdrawal");
        require(!value(call(client, "telemetryConsent", query(authority))).getBoolean("enabled"), "reader observes withdrawal");
        JSONObject next = publish(client, source, authority, true);
        require(next.getBoolean("enabled") && !next.getString("epoch").equals(epoch), "re-enable changes authority");
        long time = System.currentTimeMillis();
        JSONObject diagnostic = new JSONObject()
            .put("directory", queue.toString()).put("authority_directory", authority.toString())
            .put("consent_epoch", epoch).put("outside_session", true)
            .put("diagnostic_capture", capture.getString("session_id"))
            .put("crash_record_header", capture.getString("crash_record_header"))
            .put("diagnostic_begin_unix_ms", time).put("diagnostic_end_unix_ms", time)
            .put("message", "Synthetic authority fault").put("stack", "libsynthetic.so+0x1");
        JSONObject stale = call(client, "telemetryRecordCrash", diagnostic);
        require(stale.getBoolean("ok") && !stale.getBoolean("value"), "late old-epoch capture must refuse");
        require(!Files.exists(java.nio.file.Paths.get(capture.getString("crash_record_path"))), "refused capture must not write a record");
        JSONObject current = value(call(client, "telemetryBegin", begin.put("consent_epoch", next.getString("epoch"))));
        long currentTime = System.currentTimeMillis();
        diagnostic.put("consent_epoch", next.getString("epoch"))
            .put("diagnostic_capture", current.getString("session_id"))
            .put("crash_record_header", current.getString("crash_record_header"))
            .put("diagnostic_begin_unix_ms", currentTime).put("diagnostic_end_unix_ms", currentTime);
        JSONObject accepted = call(client, "telemetryRecordCrash", diagnostic);
        require(accepted.getBoolean("ok") && accepted.getBoolean("value"), "new epoch still records a new capture");
        require(Files.isRegularFile(java.nio.file.Paths.get(current.getString("crash_record_path"))), "current capture record exists");
    }
}
