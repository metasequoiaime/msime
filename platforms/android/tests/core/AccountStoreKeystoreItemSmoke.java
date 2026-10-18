package app.msime.android;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.stream.Stream;

/**
 * AccountStore 交给 Rust 的第 2 项（云剪贴板密钥串，迁移差异 M75）经产品的适配器（AccountStore 的 Items）读写真的 CloudClipboardKeystore：存进去的密钥串 CloudClipboardKeystore 读得出来、适配器读回的也是它；映射到别的存储时，退出登录后下一个账号会接着用上一个账号的设备私钥和剪贴板密钥。
 *
 * <p>remove(2) 不等密钥串的锁：另一条线程在锁里停住时（端到端加密驱动持着这把锁、经 Rust 核对会话），remove(2) 照样立刻返回 0、文件先留着，锁放开以后文件被删掉；同步删除会一直等到锁放开，产品里那就是 Rust 的会话锁和密钥串的锁互等。删不掉时 remove(2) 也返回 0，退出登录不因此失败。
 *
 * <p>三个 SharedPreferences 项离不开 Android，这里传 null：第 2 项要是碰了它们就会抛出。AndroidKeyStore 换成可逆的假加密；真文件位置和真加密由设备冒烟 SessionStoreDeviceSmoke 核对。
 */
public final class AccountStoreKeystoreItemSmoke {
    private static final String KEYSTORE = "v1." + "A".repeat(43) + "." + "0".repeat(32) + "." + "C".repeat(43);
    /** 冒烟里占着密钥串锁的最长时间；同步删除的 remove(2) 至少要等这么久才返回。 */
    private static final long HOLD_SECONDS = 5;

    public static void main(String[] args) throws Exception {
        Path directory = Files.createTempDirectory("msime-account-store-keystore");
        try {
            removeWaitsForNoLock(directory);
            removeSucceedsWhenDeleteFails();
        } finally {
            try (Stream<Path> files = Files.walk(directory)) {
                for (Path file : files.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
        }
        System.out.println("Android account store keystore item passed");
    }

    private static void removeWaitsForNoLock(Path directory) throws Exception {
        CloudClipboardKeystore keystore = new CloudClipboardKeystore(directory, new Toy());
        NativeClient.SessionStore items = items(keystore);
        Path file = directory.resolve(CloudClipboardKeystore.FILE_NAME);
        byte[] bytes = KEYSTORE.getBytes(StandardCharsets.UTF_8);
        check(items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == null, "nothing is stored at first");
        check(items.save(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE, bytes) == 0, "the keystore is saved through the adapter");
        check(KEYSTORE.equals(keystore.load()), "item 2 is not the cloud clipboard keystore");
        check(Arrays.equals(bytes, items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE)), "the adapter reads the keystore back");

        // 在密钥串的锁里停住：save 在锁里、写入之前调 guard，同一个值不会撞上身份冲突。
        CountDownLatch held = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        Throwable[] holderError = new Throwable[1];
        Thread holder = new Thread(() -> {
            try {
                keystore.save(KEYSTORE, () -> {
                    held.countDown();
                    if (!release.await(HOLD_SECONDS, TimeUnit.SECONDS))
                        throw new IllegalStateException("the keystore lock was held too long");
                });
            } catch (Throwable error) {
                holderError[0] = error;
            }
        }, "msime-test-keystore-holder");
        holder.setDaemon(true);
        holder.start();
        try {
            check(held.await(10, TimeUnit.SECONDS), "the holder never took the keystore lock");
            long started = System.nanoTime();
            int removed = items.remove(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE);
            long elapsed = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started);
            boolean stillHeld = release.getCount() == 1 && holder.isAlive();
            boolean keptWhileHeld = Files.exists(file);
            release.countDown();
            holder.join(10_000);
            check(removed == 0, "remove(2) returned " + removed);
            check(stillHeld && elapsed < 2_000, "remove(2) waited for the keystore lock (" + elapsed + " ms)");
            check(keptWhileHeld, "the keystore was deleted while another thread held its lock");
            check(!holder.isAlive() && holderError[0] == null, "the lock holder failed: " + holderError[0]);
        } finally {
            release.countDown();
        }

        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5);
        while (Files.exists(file) && System.nanoTime() < deadline) Thread.sleep(20);
        check(!Files.exists(file), "remove(2) did not delete the keystore once the lock was released");
        check(keystore.load() == null && items.load(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == null,
            "a removed keystore reads as absent");
    }

    /** 删不掉时 remove(2) 照样返回 0：会话已经清掉，退出登录不因为密钥串报失败（M75）。 */
    private static void removeSucceedsWhenDeleteFails() throws Exception {
        CountDownLatch attempted = new CountDownLatch(1);
        NativeClient.SessionStore items = items(new CloudClipboardE2e.Keystore() {
            @Override public String load() { return null; }

            @Override public void save(String value, CloudClipboardE2e.Guard guard) { }

            @Override public void delete() throws IOException {
                attempted.countDown();
                throw new IOException("合成的删除失败");
            }
        });
        check(items.remove(NativeClient.SessionStore.ITEM_CLIPBOARD_KEYSTORE) == 0, "a failed keystore delete fails remove(2)");
        check(attempted.await(5, TimeUnit.SECONDS), "remove(2) never tried to delete the keystore");
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
