package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

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
        System.out.println("Android sync API passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
