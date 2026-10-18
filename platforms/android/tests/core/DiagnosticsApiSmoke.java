import app.msime.android.DiagnosticsApi;
import app.msime.android.JsonPolicy;

/**
 * 「MCP 开发者访问」留在 Java 的部分：保留时长、勾选的类别和远程地址。上传前的重建（事件白名单、崩溃记录、配置快照、2 MiB 上限）和请求本身都在 Rust（#8201 迁移计划 5c），由 client-services 的 diagnostics::upload 测试覆盖。
 */
public final class DiagnosticsApiSmoke {
    public static void main(String[] arguments) throws Exception {
        check(DiagnosticsApi.Retention.fromWire("one_hour") == DiagnosticsApi.Retention.ONE_HOUR, "one_hour");
        check(DiagnosticsApi.Retention.fromWire("seven_days") == DiagnosticsApi.Retention.SEVEN_DAYS, "seven_days");
        check(DiagnosticsApi.Retention.fromWire("forever") == DiagnosticsApi.Retention.ONE_DAY, "unknown falls back to 24 h");
        check("one_day".equals(DiagnosticsApi.Retention.ONE_DAY.wire()), "ttl wire value");
        check(!new DiagnosticsApi.Include(false, false, false, false).any(), "nothing selected");
        check(new DiagnosticsApi.Include(false, false, true, false).any(), "one category selected");
        check("https://api.msime.app/mcp/s/abc".equals(DiagnosticsApi.mcpUrl("abc")), "remote address");
        check(JsonPolicy.strictLong(7L) == 7L, "diagnostic integer");
        check(JsonPolicy.strictLong(1.5d) == null, "fractional diagnostic integer is rejected");
        check(JsonPolicy.strictLong("7") == null, "numeric strings are rejected");
        check("synthetic".equals(JsonPolicy.strictString("synthetic")),
            "diagnostics identifiers accept strings");
        check(JsonPolicy.strictString(7) == null,
            "diagnostics identifiers reject numbers instead of coercing them");
        System.out.println("DiagnosticsApiSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
