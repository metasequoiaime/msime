package app.msime.android;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.EOFException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.OverlappingFileLockException;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.Objects;
import java.util.UUID;

/** Cross-process handoff for one bounded recognized voice result.
 * Audio and service credentials never enter this store. All reads, replacements,
 * expiry cleanup and claims use the same non-blocking file lock.
 */
public final class VoiceResultStore {
    public static final long LIFETIME_MILLIS = 10 * 60 * 1000L;
    public static final int MAXIMUM_CHARACTERS = 10_000;
    public static final int MAXIMUM_FILE_BYTES = 256 * 1024;
    private static final long FUTURE_TOLERANCE_MILLIS = 60 * 1000L;
    private static final int MAGIC = 0x4d565231;
    private static final int VERSION = 1;
    private static final String RESULT_NAME = "result.bin";
    private static final String LOCK_NAME = "transfer.lock";

    public enum Reason { UNAVAILABLE, BUSY, INVALID, STALE }

    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        private final Reason reason;
        Failure(Reason reason) { super(reason.name()); this.reason = reason; }
        Failure(Reason reason, Throwable cause) { super(reason.name(), cause); this.reason = reason; }
        public Reason reason() { return reason; }
    }

    public static final class Entry {
        private final UUID id;
        private final String text;
        private final long createdAtMillis;
        private final long expiresAtMillis;

        Entry(UUID id, String text, long createdAtMillis, long expiresAtMillis) {
            this.id = id;
            this.text = text;
            this.createdAtMillis = createdAtMillis;
            this.expiresAtMillis = expiresAtMillis;
        }

        public UUID id() { return id; }
        public String text() { return text; }
        public long createdAtMillis() { return createdAtMillis; }
        public long expiresAtMillis() { return expiresAtMillis; }

        @Override public boolean equals(Object value) {
            if (this == value) return true;
            if (!(value instanceof Entry other)) return false;
            return createdAtMillis == other.createdAtMillis
                && expiresAtMillis == other.expiresAtMillis && id.equals(other.id)
                && text.equals(other.text);
        }

        @Override public int hashCode() {
            return Objects.hash(id, text, createdAtMillis, expiresAtMillis);
        }
    }

    private interface LockedAction<T> { T run(Path result) throws Failure, IOException; }

    private final Path directory;

    public VoiceResultStore(Path directory) {
        if (directory == null || !directory.isAbsolute())
            throw new IllegalArgumentException("Voice result directory must be absolute");
        this.directory = directory.normalize();
    }

    public Entry read(long nowMillis) throws Failure {
        if (nowMillis < 0) throw new Failure(Reason.INVALID);
        return locked(result -> readFile(result, nowMillis));
    }

    public Entry save(String text, long nowMillis) throws Failure {
        if (nowMillis < 0 || !validText(text)) throw new Failure(Reason.INVALID);
        final long expires;
        try { expires = Math.addExact(nowMillis, LIFETIME_MILLIS); }
        catch (ArithmeticException error) { throw new Failure(Reason.INVALID, error); }
        Entry entry = new Entry(UUID.randomUUID(), text, nowMillis, expires);
        byte[] encoded = encode(entry);
        if (encoded.length > MAXIMUM_FILE_BYTES) throw new Failure(Reason.INVALID);
        return locked(result -> {
            Path temporary = Files.createTempFile(directory, "result-", ".tmp");
            try {
                try (FileOutputStream stream = new FileOutputStream(temporary.toFile())) {
                    stream.write(encoded);
                    stream.getFD().sync();
                }
                try {
                    Files.move(temporary, result, StandardCopyOption.ATOMIC_MOVE,
                        StandardCopyOption.REPLACE_EXISTING);
                } catch (AtomicMoveNotSupportedException error) {
                    throw new Failure(Reason.UNAVAILABLE, error);
                }
            } finally {
                Files.deleteIfExists(temporary);
            }
            return entry;
        });
    }

    /** Claim before synchronous editor insertion so two IME processes cannot insert twice. */
    public String consume(UUID id, long nowMillis) throws Failure {
        if (nowMillis < 0) throw new Failure(Reason.INVALID);
        if (id == null) throw new Failure(Reason.STALE);
        return locked(result -> {
            Entry entry = readFile(result, nowMillis);
            if (entry == null || !entry.id().equals(id)) throw new Failure(Reason.STALE);
            Files.delete(result);
            return entry.text();
        });
    }

    public void discard(UUID id, long nowMillis) throws Failure {
        consume(id, nowMillis);
    }

    private <T> T locked(LockedAction<T> action) throws Failure {
        try {
            rejectSymlinkComponents(directory);
            Files.createDirectories(directory);
            rejectSymlinkComponents(directory);
            if (!Files.isDirectory(directory, LinkOption.NOFOLLOW_LINKS))
                throw new Failure(Reason.UNAVAILABLE);
            Path lockPath = directory.resolve(LOCK_NAME);
            if (Files.isSymbolicLink(lockPath)
                    || (Files.exists(lockPath, LinkOption.NOFOLLOW_LINKS)
                        && !Files.isRegularFile(lockPath, LinkOption.NOFOLLOW_LINKS)))
                throw new Failure(Reason.UNAVAILABLE);
            try (FileChannel channel = FileChannel.open(lockPath, StandardOpenOption.CREATE,
                    StandardOpenOption.READ, StandardOpenOption.WRITE)) {
                FileLock lock;
                try { lock = channel.tryLock(); }
                catch (OverlappingFileLockException error) { throw new Failure(Reason.BUSY, error); }
                if (lock == null) throw new Failure(Reason.BUSY);
                try { return action.run(directory.resolve(RESULT_NAME)); }
                finally { lock.release(); }
            }
        } catch (Failure error) {
            throw error;
        } catch (IOException | SecurityException error) {
            throw new Failure(Reason.UNAVAILABLE, error);
        }
    }

    private static void rejectSymlinkComponents(Path path) throws IOException {
        Path absolute = path.toAbsolutePath().normalize();
        Path current = absolute.getRoot();
        if (current == null) throw new IOException("voice result path unavailable");
        for (Path component : absolute) {
            current = current.resolve(component);
            // macOS 的临时目录通过受信任的 /var 别名暴露。
            if (!current.toString().equals("/var") && !current.toString().equals("/tmp")
                    && Files.isSymbolicLink(current))
                throw new IOException("voice result path contains a symbolic link");
        }
    }

    private static Entry readFile(Path result, long nowMillis) throws Failure, IOException {
        if (!Files.exists(result, LinkOption.NOFOLLOW_LINKS)) return null;
        if (!Files.isRegularFile(result, LinkOption.NOFOLLOW_LINKS))
            throw new Failure(Reason.INVALID);
        byte[] bytes;
        try {
            bytes = readBounded(result);
        } catch (IOException error) {
            throw new Failure(Reason.INVALID, error);
        }
        if (bytes.length == 0) throw new Failure(Reason.INVALID);
        Entry entry = decode(bytes);
        if (entry.createdAtMillis() < 0 || entry.expiresAtMillis() <= entry.createdAtMillis()
                || !validText(entry.text())
                || entry.expiresAtMillis() - entry.createdAtMillis() != LIFETIME_MILLIS)
            throw new Failure(Reason.INVALID);
        if (entry.expiresAtMillis() <= nowMillis
                || entry.createdAtMillis() > nowMillis + FUTURE_TOLERANCE_MILLIS) {
            Files.delete(result);
            return null;
        }
        return entry;
    }

    /** Read only the accepted envelope size, even if an opened file grows after inspection. */
    private static byte[] readBounded(Path file) throws IOException {
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(MAXIMUM_FILE_BYTES);
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() + count > MAXIMUM_FILE_BYTES)
                    throw new IOException("voice result too large");
                bytes.write(buffer, 0, count);
            }
            return bytes.toByteArray();
        }
    }

    private static byte[] encode(Entry entry) throws Failure {
        final byte[] text;
        try {
            ByteBuffer encoded = StandardCharsets.UTF_8.newEncoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .encode(CharBuffer.wrap(entry.text()));
            text = new byte[encoded.remaining()];
            encoded.get(text);
        } catch (CharacterCodingException error) {
            throw new Failure(Reason.INVALID, error);
        }
        try {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(40 + text.length);
            try (DataOutputStream output = new DataOutputStream(bytes)) {
                output.writeInt(MAGIC);
                output.writeInt(VERSION);
                output.writeLong(entry.id().getMostSignificantBits());
                output.writeLong(entry.id().getLeastSignificantBits());
                output.writeLong(entry.createdAtMillis());
                output.writeLong(entry.expiresAtMillis());
                output.writeInt(text.length);
                output.write(text);
            }
            return bytes.toByteArray();
        } catch (IOException error) {
            throw new Failure(Reason.UNAVAILABLE, error);
        }
    }

    private static Entry decode(byte[] bytes) throws Failure {
        try (DataInputStream input = new DataInputStream(new ByteArrayInputStream(bytes))) {
            if (input.readInt() != MAGIC || input.readInt() != VERSION)
                throw new Failure(Reason.INVALID);
            UUID id = new UUID(input.readLong(), input.readLong());
            long created = input.readLong();
            long expires = input.readLong();
            int length = input.readInt();
            if (length < 0 || length != input.available()) throw new Failure(Reason.INVALID);
            // `readNBytes` is API 33 and this host runs from API 28. `readFully` is the
            // DataInputStream equivalent and has the same all-or-nothing contract.
            byte[] text = new byte[length];
            try {
                input.readFully(text);
            } catch (java.io.EOFException error) {
                throw new Failure(Reason.INVALID);
            }
            if (input.read() != -1) throw new Failure(Reason.INVALID);
            String decoded = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(text)).toString();
            return new Entry(id, decoded, created, expires);
        } catch (Failure error) {
            throw error;
        } catch (EOFException | CharacterCodingException error) {
            throw new Failure(Reason.INVALID, error);
        } catch (IOException error) {
            throw new Failure(Reason.INVALID, error);
        }
    }

    private static boolean validText(String text) {
        return text != null && !text.strip().isEmpty()
            && text.codePointCount(0, text.length()) <= MAXIMUM_CHARACTERS;
    }
}
