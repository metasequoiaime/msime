package app.msime.client;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.OverlappingFileLockException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.UUID;
/** Cross-process, crash-safe handoff for one cloud dictionary snapshot.
 * Native code owns snapshot decoding, staging, and activation. This class
 * owns only bounded metadata and the downloaded NDJSON file.
 */
public final class DictionarySnapshotQueue {
    public static final long MAXIMUM_SNAPSHOT_BYTES = 512L * 1024L * 1024L;
    public static final int MAXIMUM_STATE_BYTES = 65_536;
    private static final String STATE_NAME = "state.bin";
    private static final String LOCK_NAME = "state.lock";
    private static final String WORKER_LOCK_NAME = "worker.lock";
    private static final String PREFIX = "local-v1:";

    public enum Status {
        QUEUED("queued"), PREPARING("preparing"), APPLIED("applied"),
        CONFLICT("conflict"), FAILED("failed"), CANCELLED("cancelled");
        private final String wire;
        Status(String wire) { this.wire = wire; }
        boolean active() { return this == QUEUED || this == PREPARING; }
        public String wire() { return wire; }
    }

    public enum Reason { UNAVAILABLE, BUSY, INVALID, CONFLICT }

    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        private final Reason reason;
        Failure(Reason reason) { super(reason.name()); this.reason = reason; }
        Failure(Reason reason, Throwable cause) { super(reason.name(), cause); this.reason = reason; }
        public Reason reason() { return reason; }
    }

    public static final class Request {
        private final UUID id;
        private final String accountId;
        private final long cloudRevision;
        private final String expectedLocalVersion;
        private final String fileSha256;
        private final Status status;
        Request(UUID id, String accountId, long cloudRevision, String expectedLocalVersion,
                String fileSha256, Status status) {
            this.id = id;
            this.accountId = accountId;
            this.cloudRevision = cloudRevision;
            this.expectedLocalVersion = expectedLocalVersion;
            this.fileSha256 = fileSha256;
            this.status = status;
        }
        public UUID id() { return id; }
        public String accountId() { return accountId; }
        public long cloudRevision() { return cloudRevision; }
        public String expectedLocalVersion() { return expectedLocalVersion; }
        public String fileSha256() { return fileSha256; }
        public Status status() { return status; }
    }

    public static final class State {
        private final String localVersion;
        private final Request request;
        State(String localVersion, Request request) {
            this.localVersion = localVersion;
            this.request = request;
        }
        public String localVersion() { return localVersion; }
        public Request request() { return request; }
    }

    public static final class WorkerLease implements AutoCloseable {
        private final FileChannel channel;
        private final FileLock lock;
        private final Path owner;
        private boolean closed;
        private WorkerLease(FileChannel channel, FileLock lock, Path owner) {
            this.channel = channel;
            this.lock = lock;
            this.owner = owner;
        }
        @Override public void close() throws IOException {
            if (closed) return;
            closed = true;
            try { lock.release(); }
            finally { channel.close(); }
        }
    }

    @FunctionalInterface
    public interface Activation { String apply() throws Exception; }
    private interface LockedAction<T> { T run() throws Exception; }

    private final Path directory;

    public DictionarySnapshotQueue(Path directory) {
        if (directory == null || !directory.isAbsolute())
            throw new IllegalArgumentException("Snapshot queue directory must be absolute");
        this.directory = directory.normalize();
    }

    public static boolean validVersion(String value) {
        if (value == null || !value.startsWith(PREFIX)) return false;
        String[] fields = value.split(":", -1);
        if (fields.length != 3 || !validOwner(fields[1])) return false;
        return validDigest(fields[2]);
    }

    private static boolean validOwner(String value) {
        if ("legacy".equals(value)) return true;
        try { return UUID.fromString(value).toString().equals(value); }
        catch (IllegalArgumentException error) { return false; }
    }

    public static boolean validDigest(String value) {
        return value != null && value.length() == 64
            && value.chars().allMatch(c -> c >= '0' && c <= '9'
                || c >= 'a' && c <= 'f');
    }

    public State read() throws Failure { return locked(this::readUnlocked); }

    /** Publishes a native durable version and reconciles a request acknowledged by activation. */
    public void publishLocalVersion(String version) throws Failure {
        if (!validVersion(version)) throw new Failure(Reason.INVALID);
        Request applied = locked(() -> {
            State state = readUnlocked();
            Request request = state.request();
            if (request != null && request.status().active()
                    && request.id().toString().equals(version.split(":", -1)[1])) {
                request = copy(request, Status.APPLIED);
            }
            writeState(new State(version, request));
            return request != null && request.status() == Status.APPLIED ? request : null;
        });
        if (applied != null) deleteSnapshot(applied.id());
    }

    public Path filePath(UUID id) throws Failure {
        if (id == null) throw new Failure(Reason.INVALID);
        return root().resolve(id + ".ndjson");
    }

    public UUID enqueue(Path source, String accountId, long cloudRevision,
            String expectedLocalVersion, String fileSha256) throws Failure {
        if (source == null || !source.isAbsolute()
                || !Files.isRegularFile(source, LinkOption.NOFOLLOW_LINKS)
                || accountId == null || accountId.isEmpty() || accountId.length() > 128
                || cloudRevision < 0 || !validVersion(expectedLocalVersion)
                || !validDigest(fileSha256)) throw new Failure(Reason.INVALID);
        UUID id = UUID.randomUUID();
        Path incoming;
        try {
            incoming = Files.createTempFile(root(), "snapshot-", ".incoming");
            copyAndHash(source, incoming, fileSha256);
        } catch (Failure error) { throw error; }
        catch (IOException | SecurityException error) { throw new Failure(Reason.UNAVAILABLE, error); }
        Path destination = directory.resolve(id + ".ndjson");
        boolean committed = false;
        try {
            UUID result = locked(() -> {
                State before = readUnlocked();
                if (before.request() != null && before.request().status().active())
                    throw new Failure(Reason.BUSY);
                if (!expectedLocalVersion.equals(before.localVersion()))
                    throw new Failure(Reason.CONFLICT);
                try { Files.move(incoming, destination, StandardCopyOption.ATOMIC_MOVE); }
                catch (IOException error) { throw new Failure(Reason.UNAVAILABLE, error); }
                Request request = new Request(id, accountId, cloudRevision, expectedLocalVersion,
                    fileSha256, Status.QUEUED);
                writeState(new State(before.localVersion(), request));
                return id;
            });
            committed = true;
            return result;
        } finally {
            if (!committed) {
                try { Files.deleteIfExists(incoming); } catch (IOException ignored) { }
                try { Files.deleteIfExists(destination); } catch (IOException ignored) { }
            }
        }
    }

    public WorkerLease acquireWorkerLease() throws Failure {
        try {
            Path queueRoot = root();
            FileChannel channel = FileChannel.open(queueRoot.resolve(WORKER_LOCK_NAME),
                StandardOpenOption.CREATE, StandardOpenOption.READ, StandardOpenOption.WRITE);
            try {
                FileLock lock;
                try { lock = channel.tryLock(); }
                catch (OverlappingFileLockException error) { throw new Failure(Reason.BUSY, error); }
                if (lock == null) throw new Failure(Reason.BUSY);
                return new WorkerLease(channel, lock, queueRoot);
            } catch (Failure error) {
                channel.close();
                throw error;
            }
        } catch (Failure error) { throw error; }
        catch (IOException | SecurityException error) { throw new Failure(Reason.UNAVAILABLE, error); }
    }

    public Request claim(WorkerLease lease) throws Failure {
        checkLease(lease);
        return locked(() -> {
            State state = readUnlocked();
            Request request = state.request();
            if (request == null || !request.status().active()) return null;
            Request claimed = copy(request, Status.PREPARING);
            writeState(new State(state.localVersion(), claimed));
            return claimed;
        });
    }

    public boolean complete(UUID id, WorkerLease lease, String currentVersion,
            boolean alreadyApplied, Activation activation) throws Failure {
        checkLease(lease);
        if (id == null || !validVersion(currentVersion) || activation == null)
            throw new Failure(Reason.INVALID);
        boolean applied = locked(() -> {
            State state = readUnlocked();
            Request request = state.request();
            if (request == null || !id.equals(request.id())
                    || (!alreadyApplied && !request.status().active()))
                throw new Failure(Reason.CONFLICT);
            if (alreadyApplied) {
                writeState(new State(currentVersion, copy(request, Status.APPLIED)));
                return true;
            }
            if (!request.expectedLocalVersion().equals(currentVersion)) {
                writeState(new State(currentVersion, copy(request, Status.CONFLICT)));
                return false;
            }
            final String next;
            try { next = activation.apply(); }
            catch (Failure error) { throw error; }
            catch (Exception error) { throw new Failure(Reason.UNAVAILABLE, error); }
            if (!validVersion(next)) throw new Failure(Reason.INVALID);
            writeState(new State(next, copy(request, Status.APPLIED)));
            return true;
        });
        if (applied) deleteSnapshot(id);
        return applied;
    }

    public void fail(UUID id, WorkerLease lease) throws Failure {
        checkLease(lease);
        transition(id, Status.FAILED);
    }

    public void cancel(String accountId) throws Failure {
        if (accountId == null || accountId.isEmpty()) throw new Failure(Reason.INVALID);
        Request cancelled = locked(() -> {
            State state = readUnlocked();
            Request request = state.request();
            if (request == null || !accountId.equals(request.accountId()) || !request.status().active()) return null;
            Request result = copy(request, Status.CANCELLED);
            writeState(new State(state.localVersion(), result));
            return result;
        });
        if (cancelled != null) deleteSnapshot(cancelled.id());
    }

    private void transition(UUID id, Status status) throws Failure {
        if (id == null || status.active()) throw new Failure(Reason.INVALID);
        Request changed = locked(() -> {
            State state = readUnlocked();
            Request request = state.request();
            if (request == null || !id.equals(request.id()) || !request.status().active()) return null;
            Request result = copy(request, status);
            writeState(new State(state.localVersion(), result));
            return result;
        });
        if (changed != null) deleteSnapshot(changed.id());
    }

    private void checkLease(WorkerLease lease) throws Failure {
        if (lease == null || lease.closed || !directory.equals(lease.owner))
            throw new Failure(Reason.INVALID);
    }

    private Request copy(Request request, Status status) {
        return new Request(request.id(), request.accountId(), request.cloudRevision(),
            request.expectedLocalVersion(), request.fileSha256(), status);
    }

    private void deleteSnapshot(UUID id) throws Failure {
        try { Files.deleteIfExists(filePath(id)); }
        catch (IOException | SecurityException error) { throw new Failure(Reason.UNAVAILABLE, error); }
    }

    private void copyAndHash(Path source, Path destination, String expected) throws IOException, Failure {
        MessageDigest digest;
        try { digest = MessageDigest.getInstance("SHA-256"); }
        catch (NoSuchAlgorithmException error) { throw new Failure(Reason.UNAVAILABLE, error); }
        long total = 0;
        try (InputStream input = Files.newInputStream(source);
                OutputStream output = Files.newOutputStream(destination, StandardOpenOption.WRITE)) {
            byte[] buffer = new byte[65_536];
            int count;
            while ((count = input.read(buffer)) != -1) {
                total += count;
                if (total > MAXIMUM_SNAPSHOT_BYTES) throw new Failure(Reason.INVALID);
                digest.update(buffer, 0, count);
                output.write(buffer, 0, count);
            }
        }
        if (total == 0 || !hex(digest.digest()).equals(expected))
            throw new Failure(Reason.INVALID);
    }

    /** Lowercase hex. `HexFormat` is API 34 and this host runs from API 28. */
    private static String hex(byte[] bytes) {
        char[] digits = "0123456789abcdef".toCharArray();
        char[] out = new char[bytes.length * 2];
        for (int index = 0; index < bytes.length; index++) {
            int value = bytes[index] & 0xFF;
            out[index * 2] = digits[value >>> 4];
            out[index * 2 + 1] = digits[value & 0x0F];
        }
        return new String(out);
    }

    private Path root() throws Failure {
        try {
            rejectSymlinkBoundary(directory);
            Files.createDirectories(directory);
            rejectSymlinkBoundary(directory);
            if (!Files.isDirectory(directory, LinkOption.NOFOLLOW_LINKS)) throw new Failure(Reason.UNAVAILABLE);
            return directory;
        } catch (Failure error) { throw error; }
        catch (IOException | SecurityException error) { throw new Failure(Reason.UNAVAILABLE, error); }
    }

    private static void rejectSymlinkBoundary(Path path) throws IOException {
        Path absolute = path.toAbsolutePath().normalize();
        Path existing = absolute;
        while (existing != null
                && !Files.exists(existing, LinkOption.NOFOLLOW_LINKS)) {
            existing = existing.getParent();
        }
        if (existing == null || Files.isSymbolicLink(existing))
            throw new IOException("snapshot queue path contains a symbolic link");
        if (Files.exists(absolute, LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("snapshot queue directory is not a directory");
    }

    private <T> T locked(LockedAction<T> action) throws Failure {
        try {
            Path queueRoot = root();
            try (FileChannel channel = FileChannel.open(queueRoot.resolve(LOCK_NAME),
                    StandardOpenOption.CREATE, StandardOpenOption.READ, StandardOpenOption.WRITE)) {
                FileLock lock;
                try { lock = channel.tryLock(); }
                catch (OverlappingFileLockException error) { throw new Failure(Reason.BUSY, error); }
                if (lock == null) throw new Failure(Reason.BUSY);
                try { return action.run(); }
                finally { lock.release(); }
            }
        } catch (Failure error) { throw error; }
        catch (IOException | SecurityException error) { throw new Failure(Reason.UNAVAILABLE, error); }
        catch (Exception error) { throw new Failure(Reason.UNAVAILABLE, error); }
    }

    private State readUnlocked() throws Failure {
        Path stateFile = directory.resolve(STATE_NAME);
        try {
            if (!Files.exists(stateFile, LinkOption.NOFOLLOW_LINKS)) return new State(null, null);
            if (!Files.isRegularFile(stateFile, LinkOption.NOFOLLOW_LINKS))
                throw new Failure(Reason.INVALID);
            byte[] bytes = readBounded(stateFile);
            if (bytes.length == 0) throw new Failure(Reason.INVALID);
            DataInputStream input = new DataInputStream(new ByteArrayInputStream(bytes));
            if (input.readInt() != 0x4d535131 || input.readInt() != 1) throw new Failure(Reason.INVALID);
            String local = input.readBoolean() ? input.readUTF() : null;
            if (local != null && !validVersion(local)) throw new Failure(Reason.INVALID);
            Request request = input.readBoolean() ? decodeRequest(input) : null;
            if (input.available() != 0) throw new Failure(Reason.INVALID);
            return new State(local, request);
        } catch (Failure error) { throw error; }
        catch (IOException | SecurityException error) { throw new Failure(Reason.INVALID, error); }
    }

    /** Read only the metadata envelope, even if a replaced state file grows after inspection. */
    private static byte[] readBounded(Path file) throws IOException {
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(MAXIMUM_STATE_BYTES);
            byte[] buffer = new byte[4096];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() + count > MAXIMUM_STATE_BYTES)
                    throw new IOException("snapshot state too large");
                bytes.write(buffer, 0, count);
            }
            return bytes.toByteArray();
        }
    }

    private Request decodeRequest(DataInputStream input) throws IOException, Failure {
        UUID id;
        try { id = new UUID(input.readLong(), input.readLong()); }
        catch (IllegalArgumentException error) { throw new Failure(Reason.INVALID, error); }
        String accountId = input.readUTF();
        String expected = input.readUTF();
        String digest = input.readUTF();
        long revision = input.readLong();
        if (accountId.isEmpty() || accountId.length() > 128 || revision < 0
                || !validVersion(expected) || !validDigest(digest)) throw new Failure(Reason.INVALID);
        int status = input.readUnsignedByte();
        if (status >= Status.values().length) throw new Failure(Reason.INVALID);
        return new Request(id, accountId, revision, expected, digest, Status.values()[status]);
    }

    private void writeState(State state) throws Failure {
        final byte[] bytes;
        try (ByteArrayOutputStream buffer = new ByteArrayOutputStream();
                DataOutputStream output = new DataOutputStream(buffer)) {
            output.writeInt(0x4d535131);
            output.writeInt(1);
            output.writeBoolean(state.localVersion() != null);
            if (state.localVersion() != null) output.writeUTF(state.localVersion());
            output.writeBoolean(state.request() != null);
            if (state.request() != null) {
                Request request = state.request();
                output.writeLong(request.id().getMostSignificantBits());
                output.writeLong(request.id().getLeastSignificantBits());
                output.writeUTF(request.accountId());
                output.writeUTF(request.expectedLocalVersion());
                output.writeUTF(request.fileSha256());
                output.writeLong(request.cloudRevision());
                output.writeByte(request.status().ordinal());
            }
            output.flush();
            bytes = buffer.toByteArray();
        } catch (IOException error) {
            throw new Failure(Reason.INVALID, error);
        }
        if (bytes.length > MAXIMUM_STATE_BYTES) throw new Failure(Reason.INVALID);
        Path temporary = null;
        try {
            temporary = Files.createTempFile(directory, "state-", ".incoming");
            try (FileChannel channel = FileChannel.open(temporary, StandardOpenOption.WRITE)) {
                channel.write(java.nio.ByteBuffer.wrap(bytes));
                channel.force(true);
            }
            Files.move(temporary, directory.resolve(STATE_NAME), StandardCopyOption.ATOMIC_MOVE,
                StandardCopyOption.REPLACE_EXISTING);
        } catch (IOException | SecurityException error) { throw new Failure(Reason.UNAVAILABLE, error); }
        finally { if (temporary != null) try { Files.deleteIfExists(temporary); } catch (IOException ignored) { } }
    }
}
