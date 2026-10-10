import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.DictionarySnapshotWorker;
import app.msime.android.DictionarySnapshotPolicy;
import app.msime.android.JsonPolicy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import java.util.stream.Stream;

public final class DictionarySnapshotQueueSmoke {
    interface Checked { void run() throws Exception; }
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    static void fails(DictionarySnapshotQueue.Reason reason, Checked action) throws Exception {
        try { action.run(); }
        catch (DictionarySnapshotQueue.Failure error) {
            check(error.reason() == reason);
            return;
        }
        throw new AssertionError();
    }

    public static void main(String[] args) throws Exception {
        check(Boolean.TRUE.equals(JsonPolicy.strictBoolean(Boolean.TRUE)));
        check(JsonPolicy.strictBoolean("true") == null);
        check("legacy".equals(JsonPolicy.strictString("legacy")));
        check(JsonPolicy.strictString(1) == null);
        check(JsonPolicy.strictString(Boolean.TRUE) == null);
        check(DictionarySnapshotPolicy.handle(42L, -1) == 42L);
        check(DictionarySnapshotPolicy.handle(42.5, -1) == -1);
        check(DictionarySnapshotPolicy.handle(true, -1) == -1);
        check(DictionarySnapshotPolicy.handle(0L, -1) == -1);
        Path root = Files.createTempDirectory("msime-snapshot-queue-");
        try {
            Path outside = Files.createDirectory(root.resolve("outside"));
            Path linkedParent = root.resolve("linked-parent");
            Files.createSymbolicLink(linkedParent, outside);
            DictionarySnapshotQueue linkedQueue = new DictionarySnapshotQueue(
                root, linkedParent.resolve("queue"));
            fails(DictionarySnapshotQueue.Reason.UNAVAILABLE, linkedQueue::read);
            check(!Files.exists(outside.resolve("queue")));
            Files.delete(linkedParent);
            Files.createDirectory(outside.resolve("state"));
            Path linkedAncestor = root.resolve("linked-ancestor");
            Files.createSymbolicLink(linkedAncestor, outside);
            DictionarySnapshotQueue nestedQueue = new DictionarySnapshotQueue(
                root, linkedAncestor.resolve("state/queue"));
            fails(DictionarySnapshotQueue.Reason.UNAVAILABLE, nestedQueue::read);
            check(!Files.exists(outside.resolve("state/queue")));
            Files.delete(linkedAncestor);
            Path sourceRoot = root.resolve("files");
            Path sourceQueue = sourceRoot.resolve("bootstrap/state/dictionary-snapshots");
            Files.createDirectories(sourceQueue);
            Path sourceOutside = Files.createDirectory(root.resolve("source-outside"));
            Files.createSymbolicLink(sourceQueue.resolve("linked"), sourceOutside);
            Path sourcePath = sourceQueue.resolve("linked/fixture.ndjson");
            boolean sourceRejected = false;
            try {
                java.lang.reflect.Method policy = Class.forName(
                    "app.msime.android.DictionarySnapshotPathPolicy")
                    .getDeclaredMethod("privateSource", Path.class, Path.class, String.class);
                policy.setAccessible(true);
                policy.invoke(null, sourceRoot, sourceQueue, sourcePath.toString());
            } catch (java.lang.reflect.InvocationTargetException expected) {
                check(expected.getCause() instanceof java.io.IOException);
                sourceRejected = true;
            }
            check(sourceRejected);
            check(!Files.exists(sourceOutside.resolve("fixture.ndjson")));
            Path stagingOutside = Files.createDirectory(root.resolve("staging-outside"));
            Path stagingLink = root.resolve("staging-link");
            Files.createSymbolicLink(stagingLink, stagingOutside);
            boolean stagingRejected = false;
            try {
                java.lang.reflect.Method ensure = DictionarySnapshotWorker.class
                    .getDeclaredMethod("ensureSafeDirectory", Path.class);
                ensure.setAccessible(true);
                ensure.invoke(null, stagingLink);
            } catch (java.lang.reflect.InvocationTargetException expected) {
                check(expected.getCause() instanceof java.io.IOException);
                stagingRejected = true;
            }
            check(stagingRejected);
            check(!Files.exists(stagingOutside.resolve("nested")));
            Files.delete(stagingLink);
            DictionarySnapshotQueue queue = new DictionarySnapshotQueue(root, root.resolve("queue"));
            Files.createDirectories(root.resolve("queue"));
            Path outsideLock = Files.createFile(root.resolve("outside-state.lock"));
            Files.createSymbolicLink(root.resolve("queue/state.lock"), outsideLock);
            fails(DictionarySnapshotQueue.Reason.UNAVAILABLE, queue::read);
            Files.delete(root.resolve("queue/state.lock"));
            Path workerQueuePath = root.resolve("worker-queue");
            DictionarySnapshotQueue workerQueue = new DictionarySnapshotQueue(root, workerQueuePath);
            Files.createDirectories(workerQueuePath);
            Path outsideWorkerLock = Files.createFile(root.resolve("outside-worker.lock"));
            Files.createSymbolicLink(workerQueuePath.resolve("worker.lock"), outsideWorkerLock);
            fails(DictionarySnapshotQueue.Reason.UNAVAILABLE, workerQueue::acquireWorkerLease);
            Files.delete(workerQueuePath.resolve("worker.lock"));
            String version = "local-v1:legacy:" + "a".repeat(64);
            check(DictionarySnapshotQueue.validVersion(version));
            queue.publishLocalVersion(version);
            check(queue.read().localVersion().equals(version));
            Path linkedStateSource = root.resolve("outside-state.bin");
            Files.copy(root.resolve("queue/state.bin"), linkedStateSource);
            Path linkedState = root.resolve("queue/state.bin");
            Files.delete(linkedState);
            Files.createLink(linkedState, linkedStateSource);
            fails(DictionarySnapshotQueue.Reason.INVALID, queue::read);
            Files.delete(linkedState);
            queue.publishLocalVersion(version);
            UUID receipt = UUID.randomUUID();
            String receiptVersion = "local-v1:" + receipt + ":" + "b".repeat(64);
            check(DictionarySnapshotQueue.validVersion(receiptVersion));
            check(!DictionarySnapshotQueue.validVersion("local-v1:legacy:" + "A".repeat(64)));
            Path source = root.resolve("download.ndjson");
            Files.writeString(source, "record-one\nrecord-two\n");
            String digest = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256")
                .digest(Files.readAllBytes(source)));
            String account = "fixture-account";
            Path linkedSource = root.resolve("linked-download.ndjson");
            Files.createLink(linkedSource, source);
            fails(DictionarySnapshotQueue.Reason.INVALID,
                () -> queue.enqueue(linkedSource, account, 42, version, digest));
            Files.delete(linkedSource);
            fails(DictionarySnapshotQueue.Reason.INVALID,
                () -> queue.enqueue(source, account, 42, version, "0".repeat(64)));
            try (Stream<Path> entries = Files.list(root.resolve("queue"))) {
                check(entries.noneMatch(path -> path.getFileName().toString().endsWith(".incoming")));
            }
            queue.publishLocalVersion(version);
            check(queue.read().request() == null);
            UUID id = queue.enqueue(source, account, 42, version, digest);
            DictionarySnapshotQueue.State queued = queue.read();
            check(queued.request() != null && queued.request().id().equals(id)
                && queued.request().status() == DictionarySnapshotQueue.Status.QUEUED);
            check(Files.exists(queue.filePath(id)));
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                DictionarySnapshotQueue.Request claimed = queue.claim(lease, () -> account);
                check(claimed.status() == DictionarySnapshotQueue.Status.PREPARING);
                check(queue.complete(id, lease, version, false, () -> account,
                    () -> "local-v1:" + id + ":" + "b".repeat(64)));
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.APPLIED);
            check(!Files.exists(queue.filePath(id)));
            String preparingVersion = "local-v1:" + id + ":" + "b".repeat(64);
            queue.enqueue(source, account, 42, preparingVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                check(queue.claim(lease, () -> account).status() == DictionarySnapshotQueue.Status.PREPARING);
                queue.cancel(account);
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            String appliedVersion = preparingVersion;
            UUID recovered = queue.enqueue(source, account, 43, appliedVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                check(queue.claim(lease, () -> account).status() == DictionarySnapshotQueue.Status.PREPARING);
                queue.publishLocalVersion("local-v1:" + recovered + ":" + "c".repeat(64));
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.APPLIED);
            check(!Files.exists(queue.filePath(recovered)));
            fails(DictionarySnapshotQueue.Reason.CONFLICT,
                () -> queue.enqueue(source, account, 43, "local-v1:legacy:" + "c".repeat(64), digest));
            String recoveredVersion = "local-v1:" + recovered + ":" + "c".repeat(64);
            UUID failed = queue.enqueue(source, account, 43, recoveredVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                queue.claim(lease, () -> account);
                queue.fail(failed, lease);
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.FAILED);
            check(!Files.exists(queue.filePath(failed)));
            UUID second = queue.enqueue(source, account, 43, recoveredVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                queue.claim(lease, () -> account);
                check(!queue.complete(second, lease, "local-v1:legacy:" + "d".repeat(64), false, () -> account,
                    () -> "local-v1:legacy:" + "e".repeat(64)));
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CONFLICT);
            // 账号切换时只能取消原账号的请求；错误账号不能夺走或改变队列。
            queue.cancel("different-account");
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CONFLICT);
            queue.cancel(account);
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CONFLICT);
            String conflictVersion = "local-v1:legacy:" + "d".repeat(64);
            UUID cancelled = queue.enqueue(source, account, 44, conflictVersion, digest);
            queue.cancel("different-account");
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.QUEUED);
            queue.cancel(account);
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            String lateReceipt = "local-v1:" + cancelled + ":" + "e".repeat(64);
            queue.publishLocalVersion(lateReceipt);
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            check(lateReceipt.equals(queue.read().localVersion()));

            UUID stale = queue.enqueue(source, account, 45, lateReceipt, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                check(queue.claim(lease, () -> null) == null);
                check(queue.read().request().status() == DictionarySnapshotQueue.Status.QUEUED);
                check(Files.exists(queue.filePath(stale)));
                check(queue.claim(lease, () -> "") == null);
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            check(!Files.exists(queue.filePath(stale)));

            UUID replaced = queue.enqueue(source, account, 46, lateReceipt, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                check(queue.claim(lease, () -> "replacement-account") == null);
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            check(!Files.exists(queue.filePath(replaced)));

            UUID switchedDuringPreparation = queue.enqueue(source, account, 47, lateReceipt, digest);
            AtomicReference<String> currentAccount = new AtomicReference<>(account);
            AtomicBoolean activated = new AtomicBoolean();
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                check(queue.claim(lease, currentAccount::get).status() == DictionarySnapshotQueue.Status.PREPARING);
                currentAccount.set(null);
                check(!queue.complete(switchedDuringPreparation, lease, lateReceipt, false,
                    currentAccount::get, () -> {
                        activated.set(true);
                        return "local-v1:" + switchedDuringPreparation + ":" + "e".repeat(64);
                    }));
                check(!activated.get());
                check(queue.read().request().status() == DictionarySnapshotQueue.Status.PREPARING);
                check(Files.exists(queue.filePath(switchedDuringPreparation)));
                currentAccount.set("");
                check(!queue.complete(switchedDuringPreparation, lease, lateReceipt, false,
                    currentAccount::get, () -> {
                        activated.set(true);
                        return "local-v1:" + switchedDuringPreparation + ":" + "e".repeat(64);
                    }));
            }
            check(!activated.get());
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            check(!Files.exists(queue.filePath(switchedDuringPreparation)));

            // 原生快照准备和激活只认版本里的 64 位摘要；带前缀的整串会被原生侧当成越界请求拒绝。
            String digestOnly = "a".repeat(64);
            check(digestOnly.equals(DictionarySnapshotQueue.nativeVersion("local-v1:legacy:" + digestOnly)));
            check(digestOnly.equals(DictionarySnapshotQueue.nativeVersion("local-v1:" + id + ":" + digestOnly)));
            fails(DictionarySnapshotQueue.Reason.INVALID, () -> DictionarySnapshotQueue.nativeVersion(digestOnly));
            fails(DictionarySnapshotQueue.Reason.INVALID, () -> DictionarySnapshotQueue.nativeVersion(null));
            fails(DictionarySnapshotQueue.Reason.INVALID,
                () -> DictionarySnapshotQueue.nativeVersion("local-v1:legacy:" + "A".repeat(64)));

            // 本地备份恢复的请求不属于任何账号：未登录（空字符串）、已登录（真实账号 id）、账号暂时读不到（null）时都照常认领和激活，退出登录取消账号请求时也碰不到它。
            String restoreVersion = lateReceipt;
            String[][] restoreAccounts = { {""}, {"signed-in-account"}, {null} };
            for (String[] current : restoreAccounts) {
                UUID restore = queue.enqueue(source, DictionarySnapshotQueue.LOCAL_RESTORE_OWNER, 0,
                    restoreVersion, digest);
                queue.cancel(account);
                check(queue.read().request().status() == DictionarySnapshotQueue.Status.QUEUED);
                AtomicBoolean restored = new AtomicBoolean();
                String next = "local-v1:" + restore + ":" + "f".repeat(64);
                try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                    DictionarySnapshotQueue.Request claimed = queue.claim(lease, () -> current[0]);
                    check(claimed != null && claimed.id().equals(restore)
                        && claimed.status() == DictionarySnapshotQueue.Status.PREPARING);
                    check(queue.complete(restore, lease, restoreVersion, false, () -> current[0], () -> {
                        restored.set(true);
                        return next;
                    }));
                }
                check(restored.get());
                check(queue.read().request().status() == DictionarySnapshotQueue.Status.APPLIED);
                check(next.equals(queue.read().localVersion()));
                check(!Files.exists(queue.filePath(restore)));
                restoreVersion = next;
            }
            System.out.println("Android dictionary snapshot queue: atomic files, hash bounds, lease and conflict guards passed");
        } finally {
            try (Stream<Path> paths = Files.walk(root)) {
                paths.sorted(Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }
    }

}
