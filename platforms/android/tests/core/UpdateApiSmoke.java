import app.msime.android.DigestPolicy;
import app.msime.android.UpdateApi;
import app.msime.android.JsonPolicy;
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
        check("msime-android".equals(UpdateApi.apkAssetName("full")), "full asset name");
        check("msime-android-wubi".equals(UpdateApi.apkAssetName("wubi")), "edition asset name");
        rejects(() -> UpdateApi.apkAssetName("../x"), "edition ids are validated");

        // 只允许 https 与白名单主机。
        check(UpdateApi.allowedUrl("https://msime.app/api/releases?platform=android"), "msime.app allowed");
        check(UpdateApi.allowedUrl("https://release-assets.githubusercontent.com/a/b"), "asset host allowed");
        check(UpdateApi.allowedUrl(UpdateApi.MIRROR_PREFIX + "https://github.com/metasequoiaime/msime/releases/download/android-v1.1.0/msime-android.apk"), "mirror allowed");
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

        List<UpdateApi.Release> malformedTags = List.of(
            new UpdateApi.Release("android-v1.1.0", "1.1.0", false),
            new UpdateApi.Release("android bad tag", "9.0.0", false));
        check("android-v1.1.0".equals(UpdateApi.pick(malformedTags, UpdateApi.Channel.STABLE, "1.0.0").tag()),
            "invalid release tags must not hide a usable update");

        UpdateApi.Update update = UpdateApi.update(releases.get(0), "full");
        check(update.apkUrl().equals("https://github.com/metasequoiaime/msime/releases/download/android-v1.1.0/msime-android.apk"), "apk url");
        check(update.checksumUrl().equals(update.apkUrl() + ".sha256"), "checksum url");
        check(update.sumsUrl().equals("https://github.com/metasequoiaime/msime/releases/download/android-v1.1.0/SHA256SUMS"), "SHA256SUMS url");
        check(UpdateApi.update(new UpdateApi.Release("../../x", "9", false), "full") == null, "tags are validated");

        String digest = "ab".repeat(32);
        check(digest.equals(UpdateApi.parseChecksum(digest.toUpperCase(Locale.ROOT) + "  msime-android.apk\n")), "sha256sum format");
        check(UpdateApi.parseChecksum("not-a-digest") == null, "malformed checksum");
        String other = "cd".repeat(32);
        String sums = other + "  msime-android-wubi.apk\n" + digest.toUpperCase(Locale.ROOT) + "  msime-android.apk\n";
        check(digest.equals(UpdateApi.parseSums(sums, "msime-android.apk")), "SHA256SUMS line for the APK");
        check(other.equals(UpdateApi.parseSums(sums, "msime-android-wubi.apk")), "SHA256SUMS line for another edition");
        check(digest.equals(UpdateApi.parseSums("garbage\r\n" + digest + " *msime-android.apk\r\n", "msime-android.apk")),
            "binary-mode marker, CRLF and unrelated malformed lines");
        check(UpdateApi.parseSums(other + "  msime-android-wubi.apk\n", "msime-android.apk") == null, "APK not listed");
        check(UpdateApi.parseSums(digest + "  msime-android.apk.old\n" + digest + "  x/msime-android.apk\n", "msime-android.apk") == null,
            "only the exact file name matches");
        check(UpdateApi.parseSums(digest + "  msime-android.apk\n" + other + "  msime-android.apk\n", "msime-android.apk") == null,
            "conflicting lines for the same APK");
        check(UpdateApi.parseSums("zz".repeat(32) + "  msime-android.apk\n", "msime-android.apk") == null, "non-hex digest");
        check("v9.0.0".equals(JsonPolicy.strictString("v9.0.0")),
            "release metadata accepts JSON strings");
        check(JsonPolicy.strictString(7) == null,
            "numeric release metadata must not be coerced into update paths");

        // 下载：每一跳都过白名单，校验通过才留下文件。
        byte[] apk = "apk-bytes".getBytes(StandardCharsets.US_ASCII);
        String good = DigestPolicy.hex(MessageDigest.getInstance("SHA-256").digest(apk));
        Map<String, UpdateApi.Exchange> routes = new HashMap<>();
        List<String> seen = new ArrayList<>();
        UpdateApi api = new UpdateApi(url -> {
            seen.add(url);
            UpdateApi.Exchange exchange = routes.get(url);
            if (exchange == null) return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
            return exchange;
        });
        routes.put(update.checksumUrl(), new UpdateApi.Exchange(302, "https://release-assets.githubusercontent.com/sum", -1, null));
        routes.put("https://release-assets.githubusercontent.com/sum", body(good + "  msime-android.apk\n"));
        routes.put(update.apkUrl(), new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk)));
        File cache = Files.createTempDirectory("update-smoke").toFile();
        File downloaded = api.download(update, cache, null);
        check(downloaded.isFile() && downloaded.getName().equals("msime-android.apk"), "verified file kept");
        check(downloaded.getParentFile().getName().equals("updates"), "stored under cache/updates");
        check(seen.indexOf(UpdateApi.MIRROR_PREFIX + update.checksumUrl()) >= 0
                && seen.indexOf(update.checksumUrl()) > seen.indexOf(UpdateApi.MIRROR_PREFIX + update.checksumUrl())
                && seen.indexOf(update.apkUrl()) > seen.indexOf(update.checksumUrl()),
            "the mirror checksum is tried first, then the GitHub checksum and APK");
        check(seen.indexOf(UpdateApi.MIRROR_PREFIX + update.sumsUrl()) >= 0
                && seen.indexOf(UpdateApi.MIRROR_PREFIX + update.sumsUrl()) < seen.indexOf(UpdateApi.MIRROR_PREFIX + update.checksumUrl())
                && seen.indexOf(update.sumsUrl()) >= 0 && seen.indexOf(update.sumsUrl()) < seen.indexOf(update.checksumUrl()),
            "a release without SHA256SUMS falls back to <apk>.sha256 on each source");

        // 新发布：只靠 SHA256SUMS 就能核对，`<apk>.sha256` 不再被读取。
        List<String> sumsSeen = new ArrayList<>();
        UpdateApi sumsOnly = new UpdateApi(url -> {
            sumsSeen.add(url);
            if (url.equals(update.sumsUrl())) return body(other + "  msime-android-wubi.apk\n" + good + "  msime-android.apk\n");
            if (url.equals(update.apkUrl())) return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File sumsCache = Files.createTempDirectory("update-smoke-sums").toFile();
        check(good.equals(DigestPolicy.sha256Hex(sumsOnly.download(update, sumsCache, null))), "SHA256SUMS alone verifies the APK");
        check(!sumsSeen.contains(update.checksumUrl()), "<apk>.sha256 is not read when SHA256SUMS lists the APK");

        // SHA256SUMS 在，但没有列这个 APK：退回 `<apk>.sha256`。
        List<String> unlistedSeen = new ArrayList<>();
        UpdateApi unlisted = new UpdateApi(url -> {
            unlistedSeen.add(url);
            if (url.equals(update.sumsUrl())) return body(other + "  msime-android-wubi.apk\n");
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(update.apkUrl())) return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File unlistedCache = Files.createTempDirectory("update-smoke-sums-unlisted").toFile();
        check(good.equals(DigestPolicy.sha256Hex(unlisted.download(update, unlistedCache, null))),
            "a SHA256SUMS without the APK falls back to <apk>.sha256");
        check(unlistedSeen.indexOf(update.sumsUrl()) >= 0 && unlistedSeen.indexOf(update.checksumUrl()) > unlistedSeen.indexOf(update.sumsUrl()),
            "SHA256SUMS is read before <apk>.sha256");

        // SHA256SUMS 列了这个 APK 就以它为准：摘要对不上时不改用 `<apk>.sha256` 放行。
        UpdateApi sumsMismatch = new UpdateApi(url -> {
            if (url.equals(update.sumsUrl())) return body(other + "  msime-android.apk\n");
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(update.apkUrl())) return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File sumsMismatchCache = Files.createTempDirectory("update-smoke-sums-mismatch").toFile();
        try {
            sumsMismatch.download(update, sumsMismatchCache, null);
            throw new AssertionError("a SHA256SUMS mismatch must fail");
        } catch (UpdateApi.Failure expected) {
            check(!new File(sumsMismatchCache, "updates/msime-android.apk").exists(), "the mismatched APK is not kept");
        }

        // 镜像能用时完全不碰 GitHub。
        List<String> mirrorSeen = new ArrayList<>();
        UpdateApi mirrored = new UpdateApi(url -> {
            mirrorSeen.add(url);
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.apkUrl())) return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File mirroredCache = Files.createTempDirectory("update-smoke-mirror").toFile();
        check(good.equals(DigestPolicy.sha256Hex(mirrored.download(update, mirroredCache, null))), "the mirror alone delivers a verified APK");
        check(mirrorSeen.stream().allMatch(url -> url.startsWith(UpdateApi.MIRROR_PREFIX)), "GitHub is not contacted when the mirror works");

        // 镜像给的包摘要不符：删掉，换 GitHub 再下一次。
        byte[] corrupt = "apk-bytez".getBytes(StandardCharsets.US_ASCII);
        UpdateApi corrupted = new UpdateApi(url -> {
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.apkUrl())) return new UpdateApi.Exchange(200, null, corrupt.length, new ByteArrayInputStream(corrupt));
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(update.apkUrl())) return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File corruptedCache = Files.createTempDirectory("update-smoke-corrupt").toFile();
        check(good.equals(DigestPolicy.sha256Hex(corrupted.download(update, corruptedCache, null))), "a corrupt mirror copy falls back to GitHub");
        check(!new File(corruptedCache, "updates/msime-android.apk.part").exists(), "the corrupt copy is not left behind");

        // 镜像的校验文件过期时，GitHub 的 APK 必须用 GitHub 自己的校验文件核对。
        List<String> staleChecksumSeen = new ArrayList<>();
        UpdateApi staleChecksum = new UpdateApi(url -> {
            staleChecksumSeen.add(url);
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.checksumUrl()))
                return body("cd".repeat(32) + "  msime-android.apk\n");
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.apkUrl()) || url.equals(update.apkUrl()))
                return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File staleChecksumCache = Files.createTempDirectory("update-smoke-stale-checksum").toFile();
        check(good.equals(DigestPolicy.sha256Hex(staleChecksum.download(update, staleChecksumCache, null))),
            "GitHub APK is checked against its own checksum after mirror mismatch");
        check(staleChecksumSeen.contains(update.checksumUrl()), "GitHub checksum is requested for GitHub APK");
        check(!new File(staleChecksumCache, "updates/msime-android.apk.part").exists(),
            "mirror checksum mismatch leaves no partial APK");

        // 镜像 APK 不可用时，也不能拿镜像的 SHA 接受 GitHub 上不匹配的 APK。
        UpdateApi mismatchedGithub = new UpdateApi(url -> {
            if (url.equals(UpdateApi.MIRROR_PREFIX + update.checksumUrl()))
                return body(good + "  msime-android.apk\n");
            if (url.equals(update.checksumUrl()))
                return body("cd".repeat(32) + "  msime-android.apk\n");
            if (url.equals(update.apkUrl()))
                return new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        File mismatchedGithubCache = Files.createTempDirectory("update-smoke-github-mismatch").toFile();
        try {
            mismatchedGithub.download(update, mismatchedGithubCache, null);
            throw new AssertionError("GitHub APK must not use the mirror checksum");
        } catch (UpdateApi.Failure expected) {
            check(!new File(mismatchedGithubCache, "updates/msime-android.apk").exists(),
                "GitHub APK is not kept with a mismatched GitHub checksum");
        }

        // Cancelling from the progress callback must remove the partial APK.
        File cancelledCache = Files.createTempDirectory("update-smoke-cancelled").toFile();
        UpdateApi cancelApi = new UpdateApi(url -> {
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(update.apkUrl())) return new UpdateApi.Exchange(200, null, apk.length,
                new ByteArrayInputStream(apk));
            return new UpdateApi.Exchange(404, null, 0, new ByteArrayInputStream(new byte[0]));
        });
        try {
            cancelApi.download(update, cancelledCache, (done, total) -> {
                throw new java.util.concurrent.CancellationException("synthetic cancellation");
            });
            throw new AssertionError("a cancelled update must stop");
        } catch (java.util.concurrent.CancellationException expected) { }
        check(!new File(cancelledCache, "updates/msime-android.apk.part").exists(),
            "a cancelled download leaves no partial APK");

        // A partial-file symlink created after stale cleanup must not receive the APK.
        File symlinkCache = Files.createTempDirectory("update-smoke-symlink").toFile();
        File symlinkDirectory = new File(symlinkCache, "updates");
        check(symlinkDirectory.mkdirs(), "symlink test directory created");
        File external = new File(symlinkCache, "outside.apk");
        Files.writeString(external.toPath(), "sentinel");
        UpdateApi symlinkApi = new UpdateApi(url -> {
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
            if (url.equals(update.apkUrl())) {
                Files.createSymbolicLink(new File(symlinkDirectory, "msime-android.apk.part").toPath(),
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
        File symlinkTarget = new File(symlinkDirectory, "msime-android.apk");
        check(!Files.isSymbolicLink(symlinkTarget.toPath()), "update target must not be a symlink");

        // NOFOLLOW_LINKS 不防硬链接：预先放置的 .part 不得截断更新目录之外的文件。
        File hardlinkCache = Files.createTempDirectory("update-smoke-hardlink").toFile();
        File hardlinkDirectory = new File(hardlinkCache, "updates");
        check(hardlinkDirectory.mkdirs(), "hard-link test directory created");
        File hardlinkExternal = new File(hardlinkCache, "outside.apk");
        Files.writeString(hardlinkExternal.toPath(), "sentinel");
        Files.createLink(new File(hardlinkDirectory, "msime-android.apk.part").toPath(),
            hardlinkExternal.toPath());
        try {
            symlinkApi.download(update, hardlinkCache, null);
        } catch (UpdateApi.Failure expected) { }
        check("sentinel".equals(Files.readString(hardlinkExternal.toPath())),
            "update download must not follow a partial-file hard link");

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
            if (url.equals(update.checksumUrl())) return body(good + "  msime-android.apk\n");
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
        check(good.equals(DigestPolicy.sha256Hex(one)), "the shared file is intact");
        check(apkFetches.get() == 1, "the second download reuses the verified file");
        check(!new File(raceCache, "updates/msime-android.apk.part").exists(), "no partial file is left behind");

        java.nio.file.Path digestRoot = Files.createTempDirectory("digest-policy-smoke");
        try {
            java.nio.file.Path digestSource = digestRoot.resolve("source.bin");
            Files.writeString(digestSource, "synthetic");
            java.nio.file.Path digestLink = digestRoot.resolve("linked.bin");
            Files.createLink(digestLink, digestSource);
            try {
                DigestPolicy.sha256Hex(digestLink.toFile());
                throw new AssertionError("hard-linked digest input must be refused");
            } catch (java.io.IOException expected) {
                // Private digest inputs must have one directory entry.
            }
        } finally {
            try (java.util.stream.Stream<java.nio.file.Path> paths = Files.walk(digestRoot)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }

        routes.put("https://release-assets.githubusercontent.com/sum", body("cd".repeat(32) + "  msime-android.apk\n"));
        routes.put(update.apkUrl(), new UpdateApi.Exchange(200, null, apk.length, new ByteArrayInputStream(apk)));
        try {
            api.download(update, cache, null);
            throw new AssertionError("a checksum mismatch must fail");
        } catch (UpdateApi.Failure expected) {
            check(!new File(cache, "updates/msime-android.apk.part").exists(), "a failed download leaves nothing behind");
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
