import app.msime.android.CloudApi;
import app.msime.android.DiagnosticsApi;
import app.msime.android.JsonPolicy;
import java.util.ArrayList;
import java.util.List;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

public final class DiagnosticsApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // 事件只能按 P19 枚举构造：不在枚举里的种类、负时间都被丢弃。
        check(DiagnosticsApi.EventKind.values().length == 10, "P19 lists ten event kinds");
        check(DiagnosticsApi.Event.of(1, "key_down", 3) != null, "key_down is accepted");
        check(DiagnosticsApi.Event.of(1, "text", 3) == null, "an unknown kind is dropped");
        check(DiagnosticsApi.Event.of(1, "KEY_DOWN", 3) == null, "kinds are matched exactly");
        check(DiagnosticsApi.Event.of(-1, "commit", 0) == null, "a negative time is dropped");
        check(DiagnosticsApi.Event.of(5, "commit", -7).durationMs() == -1, "a negative duration means none");
        check(DiagnosticsApi.strictInteger(7L) == 7L, "diagnostic integer");
        check(DiagnosticsApi.strictInteger(1.5d) == null, "fractional diagnostic integer is rejected");
        check(DiagnosticsApi.strictInteger("7") == null, "numeric strings are rejected");

        check(DiagnosticsApi.Retention.fromWire("one_hour") == DiagnosticsApi.Retention.ONE_HOUR, "one_hour");
        check(DiagnosticsApi.Retention.fromWire("seven_days") == DiagnosticsApi.Retention.SEVEN_DAYS, "seven_days");
        check(DiagnosticsApi.Retention.fromWire("forever") == DiagnosticsApi.Retention.ONE_DAY, "unknown falls back to 24 h");
        check(!new DiagnosticsApi.Include(false, false, false, false).any(), "nothing selected");
        check(new DiagnosticsApi.Include(false, false, true, false).any(), "one category selected");

        // 请求体只有固定的键；输入事件没有耗时就不写 duration_ms，性能记录总有耗时；没选的类别不出现。
        List<DiagnosticsApi.Event> input = new ArrayList<>();
        input.add(DiagnosticsApi.Event.of(10, "key_down", -1));
        input.add(DiagnosticsApi.Event.of(12, "candidate_selected", 4));
        List<DiagnosticsApi.Event> perf = List.of(DiagnosticsApi.Event.of(20, "candidate_shown", 9));
        List<DiagnosticsApi.CrashLog> crashes = List.of(
            DiagnosticsApi.CrashLog.of("2026-10-05T05:28:00Z", "java.lang.IllegalStateException: \"x\"", "at a\nat b"));
        String body = DiagnosticsApi.requestBody("android", "1.2.3",
            new DiagnosticsApi.Sections(crashes, perf, "{\"theme\":\"system\"}", input), DiagnosticsApi.Retention.ONE_DAY);
        String expected = "{\"platform\":\"android\",\"app_version\":\"1.2.3\",\"sections\":{"
            + "\"crash_logs\":[{\"at\":\"2026-10-05T05:28:00Z\",\"message\":\"java.lang.IllegalStateException: \\\"x\\\"\","
            + "\"stack\":\"at a\\nat b\"}],"
            + "\"perf_trace\":[{\"t_ms\":20,\"kind\":\"candidate_shown\",\"duration_ms\":9}],"
            + "\"config_snapshot\":{\"theme\":\"system\"},"
            + "\"input_events\":[{\"t_ms\":10,\"kind\":\"key_down\"},{\"t_ms\":12,\"kind\":\"candidate_selected\",\"duration_ms\":4}]"
            + "},\"ttl\":\"one_day\"}";
        check(expected.equals(body), "request body layout: " + body);

        String onlyCrash = DiagnosticsApi.requestBody("android", "1", new DiagnosticsApi.Sections(
            List.of(), null, null, null), DiagnosticsApi.Retention.ONE_HOUR);
        check("{\"platform\":\"android\",\"app_version\":\"1\",\"sections\":{\"crash_logs\":[]},\"ttl\":\"one_hour\"}"
            .equals(onlyCrash), "unselected sections are left out: " + onlyCrash);

        // 崩溃摘要按 2 KiB 截断，不切开汉字。
        String longMessage = "错".repeat(1000);
        String clipped = DiagnosticsApi.CrashLog.of("", longMessage, "").message();
        check(clipped.length() == 682, "2 KiB of three-byte characters is 682 of them, got " + clipped.length());
        check(DiagnosticsApi.CrashLog.of(null, null, null).stack().isEmpty(), "missing fields become empty");
        check("synthetic".equals(JsonPolicy.strictString("synthetic")),
            "diagnostics identifiers accept strings");
        check(JsonPolicy.strictString(7) == null,
            "diagnostics identifiers reject numbers instead of coercing them");

        check("https://api.msime.app/mcp/s/abc".equals(DiagnosticsApi.mcpUrl("abc")), "remote address");

        // 删除走 DELETE /v1/users/me/diagnostics，匿名账号也可以。
        List<String> seen = new ArrayList<>();
        CloudApi api = new CloudApi((method, path, headers, raw) -> {
            seen.add(method + " " + path + " " + headers.get("Authorization"));
            return new CloudApi.Exchange(204, null, null, new byte[0]);
        }, rejected -> "", rejected -> "anon");
        new DiagnosticsApi(api).delete();
        check(seen.size() == 1 && "DELETE /v1/users/me/diagnostics Bearer anon".equals(seen.get(0)),
            "delete with the anonymous session: " + seen);

        Path root = Files.createTempDirectory("msime-diagnostics-");
        try {
            Path source = root.resolve("source.zip");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(source))) {
                output.putNextEntry(new ZipEntry("ignored.txt"));
                output.write('x');
                output.closeEntry();
            }
            Path linked = root.resolve("diagnostics.zip");
            Files.createLink(linked, source);
            try {
                DiagnosticsApi.readBundle(linked.toFile(), new DiagnosticsApi.Include(false, false, false, false));
                throw new AssertionError("hard-linked diagnostics input must be refused");
            } catch (java.io.IOException ioError) {
                // Private diagnostic archives must have one directory entry.
            }

            Path tooManyEntries = root.resolve("too-many-entries.zip");
            try (ZipOutputStream output = new ZipOutputStream(Files.newOutputStream(tooManyEntries))) {
                for (int index = 0; index < 129; index++) {
                    output.putNextEntry(new ZipEntry("ignored-" + index + ".txt"));
                    output.closeEntry();
                }
            }
            try {
                DiagnosticsApi.readBundle(tooManyEntries.toFile(),
                    new DiagnosticsApi.Include(false, false, false, false));
                throw new AssertionError("diagnostics archives must bound entry count");
            } catch (java.io.IOException bounded) {
                // A malformed or adversarial archive must stop before unbounded traversal.
            }
        } finally {
            try (java.util.stream.Stream<Path> paths = Files.walk(root)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }

        System.out.println("DiagnosticsApiSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
