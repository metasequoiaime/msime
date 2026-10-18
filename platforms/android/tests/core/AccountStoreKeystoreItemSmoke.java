package app.msime.android;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.concurrent.CancellationException;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.locks.ReentrantLock;
import java.util.stream.Stream;

/**
 * AccountStore 交给 Rust 的第 2 项（云剪贴板密钥串，迁移差异 M75）经产品的适配器（AccountStore 的 Items）读写真的 CloudClipboardKeystore：存进去的密钥串 CloudClipboardKeystore 读得出来、适配器读回的也是它；映射到别的存储时，退出登录后下一个账号会接着用上一个账号的设备私钥和剪贴板密钥。
 *
 * <p>remove(2) 是同步的，但不等密钥串的锁：另一条线程在锁里停住、等着 remove(2) 的调用方手里的一把锁时（产品里是端到端加密驱动持着密钥串的锁、经 guard 等 Rust 的会话锁，而 Rust 持着会话锁调 remove），remove(2) 照样很快返回 0，返回时文件已经删掉；那条线程随后的写入被拒绝（CancellationException），不会把旧身份写回去，下一个账号存进去的密钥串也不会被谁迟一步删掉。删不掉时 remove(2) 报失败，与文件存储相同。
 *
 * <p>三个 SharedPreferences 项离不开 Android，这里传 null：第 2 项要是碰了它们就会抛出。AndroidKeyStore 换成可逆的假加密；真文件位置和真加密由设备冒烟 SessionStoreDeviceSmoke 核对。
 */
public final class AccountStoreKeystoreItemSmoke {
    private static final String KEYSTORE = "v1." + "A".repeat(43) + "." + "0".repeat(32) + "." + "C".repeat(43);
    /** 与 KEYSTORE 同一个设备身份、换了一把剪贴板密钥：保存它时真的要写文件，不会因为内容相同而跳过。 */
    private static final String ROTATED = "v1." + "A".repeat(43) + "." + "1".repeat(32) + "." + "D".repeat(43);
    /** 下一个登录的账号生成的新身份。 */
    private static final String NEXT_ACCOUNT = "v1." + "B".repeat(43);
    /** 冒烟里占着密钥串锁的最长时间；等这把锁的 remove(2) 至少要等这么久才返回。 */
    private static final long HOLD_SECONDS = 5;

    public static void main(String[] args) throws Exception {
        Path directory = Files.createTempDirectory("msime-account-store-keystore");
        try {
            removeIsSynchronousWithoutWaitingForTheKeystoreLock(directory);
            removeReportsAFailedDelete();
        } finally {
            try (Stream<Path> files = Files.walk(directory)) {
                for (Path file : files.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
        }
        System.out.println("Android account store keystore item passed");
    }

    private static void removeIsSynchronousWithoutWaitingForTheKeystoreLock(Path directory) throws Exception {
        CloudClipboardKeystore keystore = new CloudClipboardKeystore(directory, new Toy());
        NativeClient.SessionStore items = items(keystore);
        Path file = directory.resolve(CloudClipboardKeystore.FILE_NAME);
        byte[] bytes = KEYSTORE.getBytes(StandardCharsets.UTF_8);
        check(items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == null, "nothing is stored at first");
        check(items.save(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE, bytes) == 0, "the keystore is saved through the adapter");
        check(KEYSTORE.equals(keystore.load()), "item 2 is not the cloud clipboard keystore");
        check(Arrays.equals(bytes, items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE)), "the adapter reads the keystore back");

        // session 代表 Rust 的会话锁：调 remove 的一方持着它，保存的一方在密钥串的锁里、写入之前经 guard 等它。guard 最后照常返回，相当于核对会话时登录还在。
        ReentrantLock session = new ReentrantLock();
        CountDownLatch held = new CountDownLatch(1);
        Throwable[] holderError = new Throwable[1];
        session.lock();
        Thread holder = new Thread(() -> {
            try {
                keystore.save(ROTATED, () -> {
                    held.countDown();
                    if (!session.tryLock(HOLD_SECONDS, TimeUnit.SECONDS))
                        throw new IllegalStateException("the session lock was held too long");
                    session.unlock();
                });
            } catch (Throwable error) {
                holderError[0] = error;
            }
        }, "msime-test-keystore-holder");
        holder.setDaemon(true);
        try {
            holder.start();
            check(held.await(10, TimeUnit.SECONDS), "the holder never took the keystore lock");
            long started = System.nanoTime();
            int removed = items.remove(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE);
            long elapsed = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started);
            boolean stillHeld = holder.isAlive();
            boolean goneOnReturn = !Files.exists(file);
            session.unlock();
            holder.join(10_000);
            check(removed == 0, "remove(2) returned " + removed);
            check(stillHeld && elapsed < 2_000, "remove(2) waited for the keystore lock (" + elapsed + " ms)");
            check(goneOnReturn, "remove(2) returned before the keystore was deleted");
            check(!holder.isAlive() && holderError[0] instanceof CancellationException,
                "a save that started before remove(2) was not refused: " + holderError[0]);
        } finally {
            if (session.isHeldByCurrentThread()) session.unlock();
        }
        check(!Files.exists(file), "a save that started before remove(2) wrote the old identity back");
        check(keystore.load() == null && items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == null,
            "a removed keystore reads as absent");

        // 下一个账号的密钥串：没有迟一步的删除把它删掉。
        byte[] next = NEXT_ACCOUNT.getBytes(StandardCharsets.UTF_8);
        check(items.save(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE, next) == 0, "the next account's keystore is saved");
        Thread.sleep(300);
        check(Arrays.equals(next, items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE)),
            "the next account's keystore was deleted after it was saved");
        check(items.remove(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == 0
            && items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == null, "an uncontended remove(2) deletes at once");
    }

    /** 删不掉时 remove(2) 报失败（抛出由 JNI 一侧当作失败，或返回非 0），与文件存储的会话相同：密钥串还在，下一个登录的账号会沿用这份设备身份。 */
    private static void removeReportsAFailedDelete() throws Exception {
        NativeClient.SessionStore items = items(new CloudClipboardE2e.Keystore() {
            @Override public String load() { return null; }

            @Override public void save(String value, CloudClipboardE2e.Guard guard) { }

            @Override public void delete() throws IOException {
                throw new IOException("合成的删除失败");
            }
        });
        boolean failed;
        try {
            failed = items.remove(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) != 0;
        } catch (RuntimeException reported) {
            failed = true;
        }
        check(failed, "a failed keystore delete was reported as removed");
    }

    /** 产品的适配器，只接上密钥串。 */
    private static NativeClient.SessionStore items(CloudClipboardE2e.Keystore keystore) throws Exception {
        Class<?> type = Class.forName("app.msime.android.AccountStore$Items");
        java.lang.reflect.Constructor<?> constructor = type.getDeclaredConstructor(AndroidAccountSessionStorage.class,
            AndroidAccountSessionStorage.class, AndroidAccountSessionStorage.class, CloudClipboardE2e.Keystore.class);
        constructor.setAccessible(true);
        return (NativeClient.SessionStore) constructor.newInstance(null, null, null, keystore);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    /** 可逆的假加密，与 CloudClipboardKeystoreSmoke 的做法相同。 */
    private static final class Toy implements CloudClipboardKeystore.Sealer {
        @Override public byte[] seal(byte[] plaintext) {
            byte[] sealed = plaintext.clone();
            for (int index = 0; index < sealed.length; index++) sealed[index] ^= 0x5a;
            return sealed;
        }

        @Override public byte[] open(byte[] sealed) {
            return seal(sealed);
        }
    }
}
