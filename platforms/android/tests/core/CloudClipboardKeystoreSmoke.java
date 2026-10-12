package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.stream.Stream;

/** 云剪贴板密钥串的文件存储：加密落盘、原子替换、一个安装只有一个设备身份、退出登录删除；自建中继的那份另存一个文件、按中继的绑定区分。AndroidKeyStore 换成可逆的假加密，真加密在设备冒烟里测。 */
public final class CloudClipboardKeystoreSmoke {
    private static final String IDENTITY_A = "v1." + "A".repeat(43);
    private static final String IDENTITY_B = "v1." + "B".repeat(43);
    private static final String KEYED_A = IDENTITY_A + "." + "0".repeat(32) + "." + "C".repeat(43);

    public static void main(String[] args) throws Exception {
        Path directory = Files.createTempDirectory("msime-keystore-smoke");
        try {
            run(directory);
        } finally {
            try (Stream<Path> files = Files.walk(directory)) {
                for (Path file : files.sorted(java.util.Comparator.reverseOrder()).toList()) Files.deleteIfExists(file);
            }
        }
        System.out.println("Android cloud clipboard keystore storage passed");
    }

    private static void run(Path directory) throws Exception {
        check(CloudClipboardKeystore.identity(KEYED_A).equals(IDENTITY_A), "the identity is the private-key part");
        check(CloudClipboardKeystore.identity(IDENTITY_A).equals(IDENTITY_A), "a keystore without a clipboard key has the same identity");
        check(CloudClipboardKeystore.identity("v1." + "A".repeat(42)) == null, "a short private key is no identity");
        check(CloudClipboardKeystore.identity("v2." + "A".repeat(43)) == null, "another version is no identity");
        check(CloudClipboardKeystore.identity(IDENTITY_A + "\n") == null, "control characters are refused");
        check(CloudClipboardKeystore.identity(null) == null, "null is no identity");

        Toy sealer = new Toy();
        CloudClipboardKeystore main = new CloudClipboardKeystore(directory, sealer);
        CloudClipboardKeystore ime = new CloudClipboardKeystore(directory, sealer);
        check(main.load() == null, "nothing is stored at first");

        main.save(IDENTITY_A, null);
        check(IDENTITY_A.equals(ime.load()), "the other process reads what this one wrote");
        byte[] raw = Files.readAllBytes(directory.resolve(CloudClipboardKeystore.FILE_NAME));
        check(!new String(raw, StandardCharsets.ISO_8859_1).contains("AAAA"), "the private key is not stored in plain text");
        ime.save(KEYED_A, null);
        check(KEYED_A.equals(main.load()), "the same identity may take a clipboard key");
        check(leftovers(directory).isEmpty(), "atomic writes leave no temporary files: " + leftovers(directory));

        try {
            ime.save(IDENTITY_B, null);
            throw new AssertionError("a second identity must be refused");
        } catch (CloudClipboardE2e.IdentityConflict conflict) {
            check(KEYED_A.equals(conflict.stored), "the conflict hands back the stored keystore");
        }
        check(KEYED_A.equals(main.load()), "the refused identity is not written");

        try {
            main.save(IDENTITY_A, () -> { throw new java.util.concurrent.CancellationException("account session changed"); });
            throw new AssertionError("a failing guard must stop the write");
        } catch (java.util.concurrent.CancellationException expected) {
            check(KEYED_A.equals(main.load()), "nothing is written when the login is gone");
        }

        boolean invalid = false;
        try {
            main.save("not a keystore", null);
        } catch (IllegalArgumentException expected) {
            invalid = true;
        }
        check(invalid, "a malformed keystore is refused before touching the file");

        // 解不开（AndroidKeyStore 的密钥没了）时身份已经找不回来：读作没有，允许换一个身份。
        sealer.lost = true;
        check(main.load() == null, "an unreadable file reads as absent");
        main.save(IDENTITY_B, null);
        sealer.lost = false;
        check(IDENTITY_B.equals(ime.load()), "an unreadable file may be replaced by a new identity");

        main.delete();
        check(ime.load() == null, "sign-out deletes the keystore");
        main.delete();
        ime.save(IDENTITY_A, null);
        check(IDENTITY_A.equals(main.load()), "a new identity may be stored after sign-out");
        main.delete();

        // 两个「进程」同时第一次使用：只有一个身份留下，另一个被拒绝。
        for (int round = 0; round < 20; round++) {
            ExecutorService pool = Executors.newFixedThreadPool(2);
            try {
                CountDownLatch go = new CountDownLatch(1);
                Future<Boolean> first = pool.submit(() -> race(go, main, IDENTITY_A));
                Future<Boolean> second = pool.submit(() -> race(go, ime, IDENTITY_B));
                go.countDown();
                boolean a = first.get();
                boolean b = second.get();
                check(a ^ b, "exactly one concurrent first identity wins");
                check((a ? IDENTITY_A : IDENTITY_B).equals(main.load()), "the winner is what is stored");
            } finally {
                pool.shutdownNow();
            }
            main.delete();
        }

        relay(directory, sealer, main);
    }

    /** 自建中继的密钥串：子目录第一次用到时才建、只有本人能进；只认自己那个中继的内容；换了中继不算身份冲突，直接覆盖；与账号的那份互不相干。 */
    private static void relay(Path directory, Toy sealer, CloudClipboardKeystore account) throws Exception {
        boolean refused = false;
        try {
            CloudClipboardKeystore.forRelay(directory, "", sealer);
        } catch (IllegalArgumentException expected) {
            refused = true;
        }
        check(refused, "a relay keystore needs a binding");

        Path keys = directory.resolve("cloud-clipboard-relay");
        String bindingA = "relay:" + "a".repeat(32);
        String bindingB = "relay:" + "b".repeat(32);
        CloudClipboardKeystore settingsA = CloudClipboardKeystore.forRelay(keys, bindingA, sealer);
        CloudClipboardKeystore keyboardA = CloudClipboardKeystore.forRelay(keys, bindingA, sealer);
        CloudClipboardKeystore settingsB = CloudClipboardKeystore.forRelay(keys, bindingB, sealer);
        check(settingsA.load() == null, "nothing is stored for a relay at first");
        check(java.nio.file.attribute.PosixFilePermissions.toString(Files.getPosixFilePermissions(keys)).equals("rwx------"),
            "the relay keystore directory is private");

        account.save(IDENTITY_A, null);
        check(settingsA.load() == null, "the account keystore is not the relay's");
        account.delete();

        // 这个改动之前 Rust 在同一位置写过明文 JSON：读作没有，第一次保存时被加密的那份替换。
        Path file = keys.resolve(CloudClipboardKeystore.RELAY_FILE_NAME);
        Files.writeString(file, "{\"relay\":\"https://relay.example.test\",\"space\":\"00\",\"keystore\":\"" + IDENTITY_B + "\"}");
        check(settingsA.load() == null, "a plain-text keystore from before is not read");
        settingsA.save(KEYED_A, null);
        check(KEYED_A.equals(keyboardA.load()), "the keyboard reads what the settings page wrote for the same relay");
        String raw = new String(Files.readAllBytes(file), StandardCharsets.ISO_8859_1);
        check(!raw.contains("AAAA") && !raw.contains("BBBB") && !raw.contains(bindingA) && !raw.contains("keystore"),
            "the relay keystore and its binding are sealed, the plain-text one is gone");

        check(settingsB.load() == null, "another relay or token reads as absent");
        settingsB.save(IDENTITY_B, null);
        check(IDENTITY_B.equals(settingsB.load()), "switching relays stores a new identity without a conflict");
        check(keyboardA.load() == null, "the old relay's keystore is replaced");
        try {
            CloudClipboardKeystore.forRelay(keys, bindingB, sealer).save(IDENTITY_A, null);
            throw new AssertionError("a second identity for the same relay must be refused");
        } catch (CloudClipboardE2e.IdentityConflict conflict) {
            check(IDENTITY_B.equals(conflict.stored), "the relay conflict hands back the stored keystore");
        }
        try (Stream<Path> files = Files.list(keys)) {
            List<String> names = files.map(path -> path.getFileName().toString()).sorted().toList();
            check(names.equals(List.of(CloudClipboardKeystore.RELAY_FILE_NAME, CloudClipboardKeystore.RELAY_FILE_NAME + ".lock")),
                "the relay directory holds only its keystore and lock: " + names);
        }
    }

    private static boolean race(CountDownLatch go, CloudClipboardKeystore store, String value) throws Exception {
        go.await();
        try {
            store.save(value, null);
            return true;
        } catch (CloudClipboardE2e.IdentityConflict lost) {
            return false;
        }
    }

    private static List<String> leftovers(Path directory) throws Exception {
        try (Stream<Path> files = Files.list(directory)) {
            return files.map(file -> file.getFileName().toString())
                .filter(name -> !name.equals(CloudClipboardKeystore.FILE_NAME) && !name.equals(CloudClipboardKeystore.LOCK_NAME))
                .toList();
        }
    }

    /** 可逆的假加密：逐字节异或并带一个校验字节；`lost` 为真时模拟 AndroidKeyStore 的密钥不见了。 */
    private static final class Toy implements CloudClipboardKeystore.Sealer {
        volatile boolean lost;

        @Override public byte[] seal(byte[] plaintext) {
            byte[] sealed = new byte[plaintext.length + 1];
            byte sum = 0;
            for (int index = 0; index < plaintext.length; index++) {
                sealed[index] = (byte) (plaintext[index] ^ 0x5a);
                sum += plaintext[index];
            }
            sealed[plaintext.length] = sum;
            return sealed;
        }

        @Override public byte[] open(byte[] sealed) throws Exception {
            if (lost) throw new CloudClipboardKeystore.Unreadable();
            byte[] plaintext = new byte[sealed.length - 1];
            byte sum = 0;
            for (int index = 0; index < plaintext.length; index++) {
                plaintext[index] = (byte) (sealed[index] ^ 0x5a);
                sum += plaintext[index];
            }
            if (sum != sealed[plaintext.length]) throw new CloudClipboardKeystore.Unreadable();
            return plaintext;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
