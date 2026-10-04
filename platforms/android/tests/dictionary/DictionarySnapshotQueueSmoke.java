import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.DictionarySnapshotWorker;
import app.msime.android.DictionarySnapshotPolicy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Comparator;
import java.util.HexFormat;
import java.util.UUID;
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
            UUID receipt = UUID.randomUUID();
            String receiptVersion = "local-v1:" + receipt + ":" + "b".repeat(64);
            check(DictionarySnapshotQueue.validVersion(receiptVersion));
            check(!DictionarySnapshotQueue.validVersion("local-v1:legacy:" + "A".repeat(64)));
            Path source = root.resolve("download.ndjson");
            Files.writeString(source, "record-one\nrecord-two\n");
            String digest = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256")
                .digest(Files.readAllBytes(source)));
            String account = "fixture-account";
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
                DictionarySnapshotQueue.Request claimed = queue.claim(lease);
                check(claimed.status() == DictionarySnapshotQueue.Status.PREPARING);
                check(queue.complete(id, lease, version, false,
                    () -> "local-v1:" + id + ":" + "b".repeat(64)));
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.APPLIED);
            check(!Files.exists(queue.filePath(id)));
            String appliedVersion = "local-v1:" + id + ":" + "b".repeat(64);
            UUID recovered = queue.enqueue(source, account, 43, appliedVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                check(queue.claim(lease).status() == DictionarySnapshotQueue.Status.PREPARING);
                queue.publishLocalVersion("local-v1:" + recovered + ":" + "c".repeat(64));
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.APPLIED);
            check(!Files.exists(queue.filePath(recovered)));
            fails(DictionarySnapshotQueue.Reason.CONFLICT,
                () -> queue.enqueue(source, account, 43, "local-v1:legacy:" + "c".repeat(64), digest));
            String recoveredVersion = "local-v1:" + recovered + ":" + "c".repeat(64);
            UUID failed = queue.enqueue(source, account, 43, recoveredVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                queue.claim(lease);
                queue.fail(failed, lease);
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.FAILED);
            check(!Files.exists(queue.filePath(failed)));
            UUID second = queue.enqueue(source, account, 43, recoveredVersion, digest);
            try (DictionarySnapshotQueue.WorkerLease lease = queue.acquireWorkerLease()) {
                queue.claim(lease);
                check(!queue.complete(second, lease, "local-v1:legacy:" + "d".repeat(64), false,
                    () -> "local-v1:legacy:" + "e".repeat(64)));
            }
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CONFLICT);
            queue.cancel(account);
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CONFLICT);
            String conflictVersion = "local-v1:legacy:" + "d".repeat(64);
            UUID cancelled = queue.enqueue(source, account, 44, conflictVersion, digest);
            queue.cancel(account);
            String lateReceipt = "local-v1:" + cancelled + ":" + "e".repeat(64);
            queue.publishLocalVersion(lateReceipt);
            check(queue.read().request().status() == DictionarySnapshotQueue.Status.CANCELLED);
            check(lateReceipt.equals(queue.read().localVersion()));
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
