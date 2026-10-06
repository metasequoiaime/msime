package app.msime.android;

import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.StringReader;
import java.util.ArrayList;
import java.util.List;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

public final class SyncApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // 409 + revision_conflict 才是 CAS 冲突。
        check(SyncApi.conflict(new CloudApi.Failure(409, "revision_conflict", "", 0)), "conflict");
        check(!SyncApi.conflict(new CloudApi.Failure(409, "other", "", 0)), "other 409");
        check(!SyncApi.conflict(new CloudApi.Failure(412, "revision_conflict", "", 0)), "other status");
        check(!SyncApi.conflict(null), "null failure");

        check("/v1/users/me/dictionary/changes?after=7&limit=1".equals(SyncApi.changesPath(7)), "changes path");
        check(SyncApi.changesPath(-3).endsWith("after=0&limit=1"), "negative cursor clamps");
        check("/v1/users/me/dictionary/snapshot?revision=12".equals(SyncApi.restorePath(12)), "restore path");
        try {
            SyncApi.restorePath(-1);
            throw new AssertionError("negative revision must be refused");
        } catch (IllegalArgumentException expected) {
            // 负的版本号不能拼进 URL。
        }

        // 流式请求：401 换新令牌只重试一次；没有登录不发请求；离线读成网络失败。
        String stale = "a".repeat(64);
        String fresh = "b".repeat(64);
        List<String> tokens = new ArrayList<>();
        SyncApi api = new SyncApi(null, rejected -> rejected == null ? stale : fresh, null);
        SyncApi.Exchange ok = api.streamed(token -> {
            tokens.add(token);
            return new SyncApi.Exchange(fresh.equals(token) ? 200 : 401, null, new byte[0]);
        });
        check(ok.status() == 200 && tokens.equals(List.of(stale, fresh)), "refreshed once: " + tokens);
        try {
            new SyncApi(null, rejected -> stale, null).streamed(token -> new SyncApi.Exchange(401, null, new byte[0]));
            throw new AssertionError("repeated 401 must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.signedOut(), "repeated 401 reads as signed out");
        }
        try {
            new SyncApi(null, rejected -> "", null).streamed(token -> {
                throw new AssertionError("no request without a session");
            });
            throw new AssertionError("signed out must fail");
        } catch (CloudApi.Failure failure) {
            check("signed_out".equals(failure.code), "signed out before any request");
        }
        try {
            new SyncApi(null, rejected -> stale, null).streamed(token -> {
                throw new IOException("offline");
            });
            throw new AssertionError("offline must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.network(), "offline is a network failure");
        }
        try {
            new SyncApi(null, rejected -> stale, null).streamed(token -> new SyncApi.Exchange(503, "30", new byte[0]));
            throw new AssertionError("503 must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.status == 503 && failure.retryAfterSeconds == 30, "status and Retry-After kept");
        }

        // 下载有上限，超过就中断。
        ByteArrayOutputStream sink = new ByteArrayOutputStream();
        SyncApi.BoundedStream bounded = new SyncApi.BoundedStream(sink, 4);
        bounded.write(new byte[] {1, 2, 3}, 0, 3);
        bounded.write(4);
        try {
            bounded.write(5);
            throw new AssertionError("over the limit must fail");
        } catch (IOException expected) {
            check(sink.size() == 4, "nothing past the limit is written");
        }

        // 合并路径也必须拒绝超长 NDJSON 行，不能先用 readLine 把整行分配进内存。
        try {
            SyncApi.readSnapshotLine(new BufferedReader(
                new StringReader("x".repeat(70_000) + "\n")));
            throw new AssertionError("oversized snapshot line must be refused");
        } catch (IOException expected) {
            check("snapshot line too large".equals(expected.getMessage()),
                "oversized snapshot line is bounded");
        }

        // 原生快照最多 500,000 条记录；合并路径也必须拒绝超量输入。
        SyncApi.SnapshotRecordReader records = new SyncApi.SnapshotRecordReader(
            new BufferedReader(new StringReader("{}\n".repeat(500_002))));
        for (int index = 0; index < 500_001; index++) {
            check("{}".equals(records.next()), "snapshot record within limit");
        }
        try {
            records.next();
            throw new AssertionError("excess snapshot records must be refused");
        } catch (IOException expected) {
            check("snapshot has too many records".equals(expected.getMessage()),
                "excess snapshot records are bounded");
        }

        // A hostile or stale partial-file symlink must not receive the downloaded snapshot.
        Path root = Files.createTempDirectory("msime-sync-api-");
        Path destination = root.resolve("snapshot.ndjson");
        Path outside = root.resolve("outside.ndjson");
        Path partial = root.resolve("snapshot.ndjson.partial");
        Files.writeString(outside, "sentinel", StandardOpenOption.CREATE_NEW);
        Files.createSymbolicLink(partial, outside.getFileName());
        SyncApi download = new SyncApi(null, rejected -> stale, new SyncApi.Streams() {
            @Override public SyncApi.Exchange download(String path, String token, java.io.OutputStream output)
                    throws IOException {
                output.write("{\"type\":\"header\",\"revision\":1}\n".getBytes());
                return new SyncApi.Exchange(200, null, new byte[0]);
            }

            @Override public SyncApi.Exchange upload(String path, String token, Path file,
                    String contentType) {
                throw new AssertionError("upload is not part of this smoke");
            }
        });
        try {
            download.downloadSnapshot(destination);
        } catch (Exception expected) {
            // android.jar's JVM smoke org.json stubs cannot parse the header; the
            // filesystem assertions below still exercise the write boundary.
        }
        check(!Files.isSymbolicLink(destination), "snapshot destination must not be a symlink");
        check("sentinel".equals(Files.readString(outside)),
            "snapshot download must not follow a partial-file symlink");
        Files.deleteIfExists(destination);
        Files.deleteIfExists(outside);
        Files.deleteIfExists(root);
        System.out.println("Android sync API passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
