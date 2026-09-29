import app.msime.client.VoiceResultStore;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.Comparator;
import java.util.stream.Stream;

public final class VoiceResultStoreSmoke {
    interface Checked { void run() throws Exception; }
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    static void fails(VoiceResultStore.Reason reason, Checked action) throws Exception {
        try { action.run(); }
        catch (VoiceResultStore.Failure error) {
            check(error.reason() == reason);
            return;
        }
        throw new AssertionError();
    }

    public static void main(String[] args) throws Exception {
        Path directory = Files.createTempDirectory("msime-voice-result-");
        try {
            VoiceResultStore store = new VoiceResultStore(directory);
            long now = 1_000_000L;
            Path outside = Files.createDirectory(directory.resolve("outside"));
            Path linkedParent = directory.resolve("linked-parent");
            Files.createSymbolicLink(linkedParent, outside);
            VoiceResultStore linkedStore = new VoiceResultStore(linkedParent.resolve("nested"));
            fails(VoiceResultStore.Reason.UNAVAILABLE, () -> linkedStore.save("synthetic", now));
            check(!Files.exists(outside.resolve("nested")));
            Files.delete(linkedParent);
            fails(VoiceResultStore.Reason.INVALID, () -> store.read(-1));
            fails(VoiceResultStore.Reason.INVALID, () -> store.save("   ", now));
            fails(VoiceResultStore.Reason.INVALID, () -> store.save("\ud800", now));
            fails(VoiceResultStore.Reason.INVALID,
                () -> store.save("a".repeat(VoiceResultStore.MAXIMUM_CHARACTERS + 1), now));

            VoiceResultStore.Entry first = store.save("第一条测试结果", now);
            check(store.read(now).equals(first));
            check(first.expiresAtMillis() - first.createdAtMillis()
                == VoiceResultStore.LIFETIME_MILLIS);
            VoiceResultStore.Entry second = store.save("第二条测试结果", now + 1);
            fails(VoiceResultStore.Reason.STALE, () -> store.consume(first.id(), now + 2));
            check(store.read(now + 2).equals(second));
            check(store.consume(second.id(), now + 2).equals("第二条测试结果"));
            check(store.read(now + 2) == null);

            store.save("过期测试结果", now);
            check(store.read(now + VoiceResultStore.LIFETIME_MILLIS) == null);
            store.save("未来时间测试结果", now + 60_001L);
            check(store.read(now) == null);

            Files.createDirectories(directory);
            Files.write(directory.resolve("result.bin"), new byte[] { 1, 2, 3 });
            fails(VoiceResultStore.Reason.INVALID, () -> store.read(now));
            Files.write(directory.resolve("result.bin"),
                new byte[VoiceResultStore.MAXIMUM_FILE_BYTES + 1]);
            fails(VoiceResultStore.Reason.INVALID, () -> store.read(now));
            Files.delete(directory.resolve("result.bin"));

            store.save("锁测试结果", now);
            try (FileChannel channel = FileChannel.open(directory.resolve("transfer.lock"),
                    StandardOpenOption.READ, StandardOpenOption.WRITE);
                    FileLock lock = channel.lock()) {
                check(lock.isValid());
                fails(VoiceResultStore.Reason.BUSY, () -> store.read(now));
            }

            try { new VoiceResultStore(Path.of("relative")); }
            catch (IllegalArgumentException expected) {
                System.out.println("Android voice result handoff: bounds, expiry, replacement, claim and lock passed");
                return;
            }
            throw new AssertionError();
        } finally {
            try (Stream<Path> paths = Files.walk(directory)) {
                paths.sorted(Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }
    }
}
