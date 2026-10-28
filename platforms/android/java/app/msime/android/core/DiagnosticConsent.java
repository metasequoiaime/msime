package app.msime.android.core;

import app.msime.android.SafePaths;
import java.io.File;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.UUID;

/** 镜像共享偏好的持续授权身份，供故障现场非阻塞核对；不能自行生成授权。 */
final class DiagnosticConsent {
    private static final String NAME = "telemetry-diagnostic-consent";
    private static final java.util.concurrent.locks.ReentrantLock LOCK = new java.util.concurrent.locks.ReentrantLock();
    private final Path path;

    private DiagnosticConsent(Path path) { this.path = path; }

    interface Action { void run(DiagnosticConsent consent) throws Exception; }

    /** JVM 内串行后再取进程锁；回调内的认领和撤回清理不能交错。 */
    static void locked(File files, Action action) throws Exception {
        access(files, action, false);
    }

    /** Java 致命异常不能等待另一个线程或进程的清理，锁忙时放弃本次记录。 */
    static boolean tryLocked(File files, Action action) throws Exception {
        return access(files, action, true);
    }

    private static boolean access(File files, Action action, boolean attempt) throws Exception {
        if (attempt) {
            if (LOCK.isHeldByCurrentThread()) return false;
            if (!LOCK.tryLock()) return false;
        } else LOCK.lock();
        try {
            Path root = files.toPath();
            SafePaths.ensureDirectory(root);
            Path lockPath = root.resolve(NAME + ".lock");
            SafePaths.rejectSymlinkComponents(lockPath);
            try (FileChannel channel = FileChannel.open(lockPath, StandardOpenOption.CREATE,
                    StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS)) {
                FileLock lock = attempt ? channel.tryLock() : channel.lock();
                if (lock == null) return false;
                try {
                    if (!lock.isValid() || !SafePaths.isSingleLink(lockPath)) throw new IOException("diagnostic lock");
                    action.run(new DiagnosticConsent(root.resolve(NAME)));
                    return true;
                } finally { lock.release(); }
            }
        } finally { LOCK.unlock(); }
    }

    State observe(State next) throws IOException {
        SafePaths.rejectSymlinkComponents(path);
        if (next == null) {
            Files.deleteIfExists(path);
            return null;
        }
        State current = current();
        if (current != null && current.epoch.equals(next.epoch) && current.since == next.since) return current;
        Path temporary = Files.createTempFile(path.getParent(), NAME, ".tmp");
        try {
            byte[] bytes = (next.epoch + "|" + next.since).getBytes(StandardCharsets.US_ASCII);
            try (FileChannel output = FileChannel.open(temporary, StandardOpenOption.WRITE,
                    StandardOpenOption.TRUNCATE_EXISTING, LinkOption.NOFOLLOW_LINKS)) {
                ByteBuffer buffer = ByteBuffer.wrap(bytes);
                while (buffer.hasRemaining()) output.write(buffer);
                output.force(true);
            }
            Files.move(temporary, path, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } finally { Files.deleteIfExists(temporary); }
        return next;
    }

    State current() throws IOException {
        SafePaths.rejectSymlinkComponents(path);
        if (!Files.exists(path, LinkOption.NOFOLLOW_LINKS)) return null;
        if (!Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS) || !SafePaths.isSingleLink(path))
            throw new IOException("diagnostic consent file");
        try (FileChannel input = FileChannel.open(path, StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)) {
            if (input.size() > 64) throw new IOException("diagnostic consent size");
            ByteBuffer bytes = ByteBuffer.allocate(65);
            while (bytes.hasRemaining() && input.read(bytes) != -1) { /* 有界读取，不追随替换后的目录项。 */ }
            String[] fields = new String(bytes.array(), 0, bytes.position(), StandardCharsets.US_ASCII).split("\\|", -1);
            if (fields.length != 2 || !canonicalUuid(fields[0])) throw new IOException("diagnostic consent identity");
            try {
                long since = Long.parseLong(fields[1]);
                if (since <= 0) throw new IOException("diagnostic consent time");
                return new State(fields[0], since);
            } catch (NumberFormatException error) { throw new IOException("diagnostic consent time", error); }
        }
    }

    private static boolean canonicalUuid(String value) {
        try { return UUID.fromString(value).toString().equals(value); }
        catch (IllegalArgumentException error) { return false; }
    }

    static final class State {
        final String epoch;
        final long since;
        State(String epoch, long since) {
            if (!canonicalUuid(epoch) || since <= 0) throw new IllegalArgumentException("diagnostic consent identity");
            this.epoch = epoch;
            this.since = since;
        }

        String summary(String capture, String version, String component) {
            if (!canonicalUuid(capture) || !version.matches("[A-Za-z0-9._-]{1,64}") || !component(component)) return null;
            String text = "v2|" + epoch + "|" + capture + "|" + version + "|" + component;
            // Android 上限为 128 字节；不能截断构建版本或 UUID 来勉强塞入。
            return text.length() <= 128 ? text : null;
        }

        Identity accept(byte[] bytes, long occurred, long now) {
            if (bytes == null || bytes.length > 128 || occurred < since || occurred > now || occurred <= 0) return null;
            String[] fields = new String(bytes, StandardCharsets.US_ASCII).split("\\|", -1);
            if (fields.length != 5 || !fields[0].equals("v2") || !fields[1].equals(epoch)
                    || !canonicalUuid(fields[2]) || !fields[3].matches("[A-Za-z0-9._-]{1,64}")
                    || !component(fields[4])) return null;
            return new Identity(fields[2], fields[3], fields[4]);
        }

        private static boolean component(String value) { return value.equals("application") || value.equals("keyboard"); }
    }

    static final class Identity {
        final String capture;
        final String version;
        final String component;
        Identity(String capture, String version, String component) {
            this.capture = capture;
            this.version = version;
            this.component = component;
        }
    }
}
