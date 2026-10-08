import app.msime.android.LocalBackupPolicy;
import app.msime.android.LocalBackupPolicy.Compatibility;
import app.msime.android.LocalBackupPolicy.Restored;
import java.time.LocalDateTime;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** 本地备份包的格式规则（#5659）：文件名带应用名和版本号、能不能恢复、设置文档的字段类型、恢复结果的说明。 */
public final class LocalBackupPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static void equal(Object actual, Object expected, String message) {
        if (!expected.equals(actual)) throw new AssertionError(message + ": expected <" + expected + "> but was <" + actual + ">");
    }

    public static void main(String[] args) {
        LocalDateTime now = LocalDateTime.of(2026, 10, 8, 9, 5);
        equal(LocalBackupPolicy.fileName("水杉输入法", "0.2.2", now), "水杉输入法-0.2.2-20261008-0905.zip",
            "file name carries the app name, the version and the export time");
        equal(LocalBackupPolicy.fileName("水杉 五笔", "1.0/beta", now), "水杉_五笔-1.0_beta-20261008-0905.zip",
            "characters a file name cannot hold become underscores");
        equal(LocalBackupPolicy.fileName("", null, now), "msime-0-20261008-0905.zip", "missing name parts fall back");

        equal(LocalBackupPolicy.compatibility("msime-android-backup", 1), Compatibility.OK, "current format");
        equal(LocalBackupPolicy.compatibility("msime-android-backup", 2), Compatibility.NEWER_FORMAT,
            "a newer format asks for an update instead of half-restoring");
        equal(LocalBackupPolicy.compatibility("msime-android-backup", "1"), Compatibility.NOT_A_BACKUP,
            "a string version is not trusted");
        equal(LocalBackupPolicy.compatibility("msime-diagnostics", 1), Compatibility.NOT_A_BACKUP,
            "a diagnostics bundle is not a backup");
        equal(LocalBackupPolicy.compatibility(null, null), Compatibility.NOT_A_BACKUP, "no manifest");

        check(LocalBackupPolicy.backsUpLocalSetting("platform.android.keyboard_height_adjustment"),
            "device-local layout settings travel with the backup");
        check(LocalBackupPolicy.backsUpLocalSetting("platform.android.incognito"), "privacy mode travels with the backup");
        check(!LocalBackupPolicy.backsUpLocalSetting("platform.android.voice_contribute_audio"),
            "voice upload consent is given on each device, never restored");
        check(!LocalBackupPolicy.backsUpLocalSetting("platform.android.developer.input_log"),
            "a restored backup does not start an input log");
        check(!LocalBackupPolicy.backsUpLocalSetting("platform.android.developer.mcp_input_events"),
            "MCP log scopes stay on the device");
        check(!LocalBackupPolicy.backsUpLocalSetting(null), "no key");

        Map<String, Object> settings = new LinkedHashMap<>();
        settings.put("input.learning", true);
        settings.put("input.frequency_trigger_count", 3);
        settings.put("candidate.font_scale", 1.25);
        settings.put("input.schema", "quanpin");
        settings.put("weird", List.of());
        Map<String, String> types = LocalBackupPolicy.fieldTypes(settings);
        equal(types.get("input.learning"), "boolean", "boolean field");
        equal(types.get("input.frequency_trigger_count"), "integer", "integer field");
        equal(types.get("candidate.font_scale"), "number", "number field");
        equal(types.get("input.schema"), "string", "string field");
        check(!types.containsKey("weird"), "values of other types are not declared");
        Map<String, Object> many = new LinkedHashMap<>();
        for (int index = 0; index < 513; index++) many.put("key." + index, true);
        check(LocalBackupPolicy.fieldTypes(many) == null, "more fields than client-core accepts");

        equal(LocalBackupPolicy.summary(new Restored(true, 2, 5, 120, 0, List.of())),
            "已恢复设置、2 个自定义皮肤、5 条常用语；120 个词会在键盘空闲时陆续写入词库。", "full restore");
        equal(LocalBackupPolicy.summary(new Restored(false, 0, 0, 0, 0, List.of())),
            "备份里没有需要恢复的新内容。", "nothing new");
        equal(LocalBackupPolicy.summary(new Restored(true, 0, 0, 10, 2, List.of("常用语"))),
            "已恢复设置；10 个词会在键盘空闲时陆续写入词库；2 个词无法导入，已跳过；常用语没有恢复，请重试。",
            "partial restore names what failed");
        System.out.println("Android local backup policy passed");
    }
}
