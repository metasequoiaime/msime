import app.msime.android.DeviceDataApi;
import java.nio.charset.StandardCharsets;
import java.time.LocalDate;
import java.util.Base64;
import java.util.List;
import java.util.UUID;

/**
 * 「我的」的纯逻辑：头像、云端数据分区、大小与时间的写法。请求都经 Rust（#8201 迁移计划 5c），令牌、401 后的重试和导出的原子替换由 client-services 和 host-api 的测试覆盖，这里只核对 Java 一侧留下的规则。
 */
public final class DeviceDataApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // Rust 报的「要求最近登录」才弹重新登录，其他失败不弹。
        check(DeviceDataApi.recentLoginRequired("account_recent_login_required"), "recent login is recognised");
        check(!DeviceDataApi.recentLoginRequired("account_unauthorized"), "a lost session is not a recent-login demand");
        check(!DeviceDataApi.recentLoginRequired(null), "no code");

        // 删除云端数据连同这台设备匿名账号的语音样本（Rust 的 data_delete 认 voice）。
        check(DeviceDataApi.DELETABLE_SECTIONS.equals(List.of("preferences", "dictionary", "phrases", "clipboard", "voice")),
            "every deletable section, voice included, in dialog order");

        // 头像按文件头认格式。
        byte[] png = {(byte) 0x89, 'P', 'N', 'G', 13, 10, 26, 10};
        byte[] jpeg = {(byte) 0xFF, (byte) 0xD8, (byte) 0xFF, 0};
        check("image/png".equals(DeviceDataApi.avatarType(png)), "png");
        check("image/jpeg".equals(DeviceDataApi.avatarType(jpeg)), "jpeg");
        check(DeviceDataApi.avatarType(new byte[] {'G', 'I', 'F', '8'}) == null, "gif is refused");
        check(DeviceDataApi.avatarType(null) == null, "missing image");
        // 上传只收 PNG、JPEG；显示还认 WebP：Rust 下载时声明接受它，Google 的头像主机多半就回 WebP。
        byte[] webp = {'R', 'I', 'F', 'F', 4, 0, 0, 0, 'W', 'E', 'B', 'P', 'V', 'P', '8', ' '};
        check(DeviceDataApi.avatarType(webp) == null, "webp is not uploadable");
        check("image/webp".equals(DeviceDataApi.displayAvatarType(webp)), "webp is displayable");
        check("image/jpeg".equals(DeviceDataApi.displayAvatarType(jpeg)), "jpeg is displayable");
        check(DeviceDataApi.displayAvatarType(new byte[] {'R', 'I', 'F', 'F', 4, 0, 0, 0, 'W', 'A', 'V', 'E'}) == null,
            "another RIFF container is refused");
        check(DeviceDataApi.displayAvatarType(null) == null, "missing image to display");

        // Rust 交回的头像是 data: URL，只认 PNG、JPEG 和 WebP，文件头也要是这三种之一。
        String encodedPng = Base64.getEncoder().encodeToString(png);
        check(java.util.Arrays.equals(png, DeviceDataApi.avatarBytes("data:image/png;base64," + encodedPng)),
            "a PNG data URL decodes to its bytes");
        check(java.util.Arrays.equals(jpeg, DeviceDataApi.avatarBytes(
            "data:image/jpeg;base64," + Base64.getEncoder().encodeToString(jpeg))), "a JPEG data URL decodes");
        check(java.util.Arrays.equals(webp, DeviceDataApi.avatarBytes(
            "data:image/webp;base64," + Base64.getEncoder().encodeToString(webp))), "a WebP data URL decodes");
        check(DeviceDataApi.avatarBytes("data:image/gif;base64," + encodedPng) == null, "another media type is refused");
        check(DeviceDataApi.avatarBytes("https://media.msime.app/a.png") == null, "a remote URL is not fetched here");
        check(DeviceDataApi.avatarBytes("data:image/png;base64,%%%") == null, "malformed base64 is refused");
        check(DeviceDataApi.avatarBytes("data:image/png;base64,"
            + Base64.getEncoder().encodeToString("not an image".getBytes(StandardCharsets.UTF_8))) == null,
            "bytes that are not an image are refused");
        check(DeviceDataApi.avatarBytes("data:image/png;base64," + "A".repeat(2 * 1024 * 1024)) == null,
            "an avatar larger than 1 MiB is refused before decoding");
        check(DeviceDataApi.avatarBytes(null) == null, "no avatar");

        check("512 B".equals(DeviceDataApi.formatBytes(512)), "bytes");
        check("12.4 MB".equals(DeviceDataApi.formatBytes(13_002_342)), "megabytes with one decimal");
        check("1.0 KB".equals(DeviceDataApi.formatBytes(1024)), "kilobytes");
        check("0 B".equals(DeviceDataApi.formatBytes(-5)), "negative is zero");

        long now = 1_800_000_000_000L;
        check("".equals(DeviceDataApi.relativeTime(now, 0)), "unknown time is blank");
        check("刚刚".equals(DeviceDataApi.relativeTime(now, now - 30_000)), "within a minute");
        check("2 分钟前".equals(DeviceDataApi.relativeTime(now, now - 150_000)), "minutes");
        check("3 小时前".equals(DeviceDataApi.relativeTime(now, now - 3 * 3_600_000L)), "hours");
        check("2 天前".equals(DeviceDataApi.relativeTime(now, now - 50 * 3_600_000L)), "days");
        check("msime-data-2026-10-05.zip".equals(DeviceDataApi.exportFileName(LocalDate.of(2026, 10, 5))),
            "export file name");
        LocalDate exportDay = LocalDate.of(2026, 10, 5);
        String firstArchive = DeviceDataApi.exportCacheFileName(exportDay,
            UUID.fromString("00000000-0000-0000-0000-000000000001"));
        String nextArchive = DeviceDataApi.exportCacheFileName(exportDay,
            UUID.fromString("00000000-0000-0000-0000-000000000002"));
        check(!firstArchive.equals(nextArchive), "separate exports cannot reuse a shared URI path");
        check(firstArchive.startsWith("msime-data-2026-10-05-") && firstArchive.endsWith(".zip"),
            "the private export path remains a dated zip archive");
        check(DeviceDataApi.validDeleteSections(List.of("preferences", "phrases")),
            "unique deletable sections are accepted");
        check(!DeviceDataApi.validDeleteSections(List.of("phrases", "phrases")),
            "duplicate deletable sections are refused");
        check(!DeviceDataApi.validDeleteSections(List.of("unknown")),
            "unknown deletable sections are refused");
        check(!DeviceDataApi.validDeleteSections(List.of()), "empty deletable sections are refused");
        check(!DeviceDataApi.validDeleteSections(null), "missing deletable sections are refused");
        System.out.println("Android device data API passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
