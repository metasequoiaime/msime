import app.msime.android.UpdateApi;
import java.io.ByteArrayInputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

public final class UpdateApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // 资产名与 edition_android.py 的 apk_name 一致。
        check("msime-client".equals(UpdateApi.apkAssetName("full")), "full asset name");
        check("msime-client-wubi".equals(UpdateApi.apkAssetName("wubi")), "edition asset name");
        rejects(() -> UpdateApi.apkAssetName("../x"), "edition ids are validated");

        // 只允许 https 与白名单主机。
        check(UpdateApi.allowedUrl("https://msime.app/api/releases?platform=android"), "msime.app allowed");
        check(UpdateApi.allowedUrl("https://release-assets.githubusercontent.com/a/b"), "asset host allowed");
        check(!UpdateApi.allowedUrl("http://github.com/x"), "plain http refused");
        check(!UpdateApi.allowedUrl("https://evil.example/x"), "other hosts refused");
        check(!UpdateApi.allowedUrl("https://github.com.evil.example/x"), "suffix tricks refused");
        check(!UpdateApi.allowedUrl("https://user@github.com/x"), "user info refused");
        check(!UpdateApi.allowedUrl("https://github.com:8443/x"), "odd ports refused");

        // 版本比较：数字逐段比，正式版比同号的预发布版新。
        check(UpdateApi.compareVersions("1.10.0", "1.9.9") > 0, "numeric segments");
        check(UpdateApi.compareVersions("1.0.0", "1.0.0-rc.1") > 0, "release beats prerelease");
        check(UpdateApi.compareVersions("0.50.0-build.14", "0.50.0-build.9") > 0, "numeric prerelease ids");
        check(UpdateApi.compareVersions("v1.0", "1.0.0") == 0, "missing segments are zero");
        check(UpdateApi.compareVersions("0.1.0-dev", "0.1.0") < 0, "dev build is older than its release");
        check(UpdateApi.compareVersions("1.9000000000000000000", "1.10000000000000000000") < 0,
            "large numeric segments keep numeric ordering");

        List<UpdateApi.Release> releases = List.of(
            new UpdateApi.Release("android-v1.1.0", "1.1.0", false),
            new UpdateApi.Release("android-v1.2.0-rc.1", "1.2.0-rc.1", true),
            new UpdateApi.Release("android-v1.0.0", "1.0.0", false));
        check("1.1.0".equals(UpdateApi.pick(releases, UpdateApi.Channel.STABLE, "1.0.0").version()), "stable skips prereleases");
        check("1.2.0-rc.1".equals(UpdateApi.pick(releases, UpdateApi.Channel.PREVIEW, "1.0.0").version()), "preview sees prereleases");
        check(UpdateApi.pick(releases, UpdateApi.Channel.STABLE, "1.1.0") == null, "up to date");
        check(UpdateApi.Channel.fromId("nonsense") == UpdateApi.Channel.STABLE, "unknown channel is stable");

        UpdateApi.Update update = UpdateApi.update(releases.get(0), "full");
        check(update.apkUrl().equals("https://github.com/metasequoiaime/msime/releases/download/android-v1.1.0/msime-client.apk"), "apk url");
        check(update.checksumUrl().equals(update.apkUrl() + ".sha256"), "checksum url");
        check(UpdateApi.update(new UpdateApi.Release("../../x", "9", false), "full") == null, "tags are validated");

        String digest = "ab".repeat(32);
        check(digest.equals(UpdateApi.parseChecksum(digest.toUpperCase(Locale.ROOT) + "  msime-client.apk\n")), "sha256sum format");
        check(UpdateApi.parseChecksum("not-a-digest") == null, "malformed checksum");
        java.lang.reflect.Method strictString = UpdateApi.class.getDeclaredMethod("strictString", Object.class);
        strictString.setAccessible(true);
        check("v9.0.0".equals(strictString.invoke(null, "v9.0.0")),
            "release metadata accepts JSON strings");
        check(strictString.invoke(null, 7) == null,
            "numeric release metadata must not be coerced into update paths");

        // 下载：每一跳都过白名单，校验通过才留下文件。
        byte[] apk = "apk-bytes".getBytes(StandardCharsets.US_ASCII);
        String good = hex(MessageDigest.getInstance("SHA-256").digest(apk));
        Map<String, UpdateApi.Exchange> routes = new HashMap<>();
        List<String> seen = new ArrayList<>();
        UpdateApi api = new UpdateApi(url -> {
            seen.add(url);
            UpdateApi.Exchange exchange = routes.get(url);
            if (exchange == null) return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
            return exchange;
        });
        routes.put(update.checksumUrl(), new UpdateApi.Exchange(302, "https://release-assets.githubusercontent.com/sum", -1, null));
        routes.put("https://release-assets.githubusercontent.com/sum", body(good + "  msime-client.apk\n"));
        routes.put(update.apkUrl(), new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk)));
        File cache = Files.createTempDirectory("update-smoke").toFile();
        File downloaded = api.download(update, cache, null);
        check(downloaded.isFile() && downloaded.getName().equals("msime-client.apk"), "verified file kept");
        check(downloaded.getParentFile().getName().equals("updates"), "stored under cache/updates");

        // A partial-file symlink created after stale cleanup must not receive the APK.
        File symlinkCache = Files.createTempDirectory("update-smoke-symlink").toFile();
        File symlinkDirectory = new File(symlinkCache, "updates");
        check(symlinkDirectory.mkdirs(), "symlink test directory created");
        File external = new File(symlinkCache, "outside.apk");
        Files.writeString(external.toPath(), "sentinel");
        UpdateApi symlinkApi = new UpdateApi(url -> {
            if (url.equals(update.checksumUrl())) return body(good + "  msime-client.apk\n");
            if (url.equals(update.apkUrl())) {
                Files.createSymbolicLink(new File(symlinkDirectory, "msime-client.apk.part").toPath(),
                    symlinkDirectory.toPath().relativize(external.toPath()));
                return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            }
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        try {
            symlinkApi.download(update, symlinkCache, null);
        } catch (UpdateApi.Failure expected) {
            // Refusing the symlink is the expected result.
        }
        check("sentinel".equals(Files.readString(external.toPath())),
            "update download must not follow a partial-file symlink");
        File symlinkTarget = new File(symlinkDirectory, "msime-client.apk");
        check(!Files.isSymbolicLink(symlinkTarget.toPath()), "update target must not be a symlink");

        // The updates directory itself must not redirect writes outside the cache.
        File directorySymlinkCache = Files.createTempDirectory("update-smoke-directory-link").toFile();
        File directoryOutside = Files.createTempDirectory("update-smoke-directory-outside").toFile();
        File linkedUpdates = new File(directorySymlinkCache, "updates");
        check(Files.createSymbolicLink(linkedUpdates.toPath(), directoryOutside.toPath()) != null,
            "updates directory symlink created");
        try {
            symlinkApi.download(update, directorySymlinkCache, null);
            throw new AssertionError("a symlinked updates directory must fail");
        } catch (UpdateApi.Failure expected) { }
        check(!new File(directoryOutside, update.fileName()).exists(),
            "a symlinked updates directory must not receive the APK");

        // 关于页和每日任务同时下载：排队进行，后一次直接用前一次已经核对过的文件，不互删 .part、也不再下一遍。
        java.util.concurrent.atomic.AtomicInteger apkFetches = new java.util.concurrent.atomic.AtomicInteger();
        UpdateApi racing = new UpdateApi(url -> {
            if (url.equals(update.checksumUrl())) return body(good + "  msime-client.apk\n");
            if (url.equals(update.apkUrl())) {
                apkFetches.incrementAndGet();
                return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            }
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File raceCache = Files.createTempDirectory("update-smoke-race").toFile();
        java.util.concurrent.ExecutorService pool = java.util.concurrent.Executors.newFixedThreadPool(2);
        java.util.concurrent.Future<File> first = pool.submit(() -> racing.download(update, raceCache, null));
        java.util.concurrent.Future<File> second = pool.submit(() -> racing.download(update, raceCache, null));
        File one = first.get();
        File two = second.get();
        pool.shutdown();
        check(one.equals(two) && one.isFile(), "concurrent downloads end with the same verified file");
        check(good.equals(UpdateApi.sha256Hex(one)), "the shared file is intact");
        check(apkFetches.get() == 1, "the second download reuses the verified file");
        check(!new File(raceCache, "updates/msime-client.apk.part").exists(), "no partial file is left behind");

        routes.put("https://release-assets.githubusercontent.com/sum", body("cd".repeat(32) + "  msime-client.apk\n"));
        routes.put(update.apkUrl(), new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk)));
        try {
            api.download(update, cache, null);
            throw new AssertionError("a checksum mismatch must fail");
        } catch (UpdateApi.Failure expected) {
            check(!new File(cache, "updates/msime-client.apk.part").exists(), "a failed download leaves nothing behind");
        }

        routes.put(update.checksumUrl(), new UpdateApi.Exchange(302, "https://evil.example/sum", -1, null));
        seen.clear();
        try {
            api.download(update, cache, null);
            throw new AssertionError("a redirect off the allow list must fail");
        } catch (UpdateApi.Failure expected) {
            check(seen.stream().noneMatch(url -> url.contains("evil.example")), "the off-list host is never contacted");
        }
        System.out.println("UpdateApiSmoke ok");
    }

    private static UpdateApi.Exchange body(String text) {
        byte[] bytes = text.getBytes(StandardCharsets.US_ASCII);
        return new UpdateApi.Exchange(200, null, bytes.length, new ByteArrayInputStream(bytes));
    }

    private static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder();
        for (byte value : bytes) out.append(String.format(Locale.ROOT, "%02x", value & 0xff));
        return out.toString();
    }

    private static void rejects(Runnable action, String message) {
        try {
            action.run();
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError(message);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
