import app.msime.android.CloudApi;
import app.msime.android.DiagnosticsApi;
import java.util.ArrayList;
import java.util.List;

public final class DiagnosticsApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // 事件只能按 P19 枚举构造：不在枚举里的种类、负时间都被丢弃。
        check(DiagnosticsApi.EventKind.values().length == 10, "P19 lists ten event kinds");
        check(DiagnosticsApi.Event.of(1, "key_down", 3) != null, "key_down is accepted");
        check(DiagnosticsApi.Event.of(1, "text", 3) == null, "an unknown kind is dropped");
        check(DiagnosticsApi.Event.of(1, "KEY_DOWN", 3) == null, "kinds are matched exactly");
        check(DiagnosticsApi.Event.of(-1, "commit", 0) == null, "a negative time is dropped");
        check(DiagnosticsApi.Event.of(5, "commit", -7).durationMs() == -1, "a negative duration means none");

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

        System.out.println("DiagnosticsApiSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
