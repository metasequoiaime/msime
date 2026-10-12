package app.msime.android;

import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.io.File;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.PosixFilePermissions;
import java.security.KeyStore;
import java.util.Arrays;
import javax.crypto.AEADBadTagException;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/**
 * 云剪贴板的本机密钥串：设备私钥和当前剪贴板密钥，由共享驱动交来的不透明字符串（`docs/cloud-clipboard-e2e.md` 的「密钥串的存放位置」）。
 *
 * <p>存在应用私有目录下的 `cloud-clipboard-keystore`，和账号会话一样先用 AndroidKeyStore 里的 AES-GCM 密钥加密（{@link AndroidAccountSessionStorage}），密钥不出 AndroidKeyStore。主进程（原生设置页、账号页、Tauri 设置页）和 `:ime` 进程（键盘）读写的是这同一个文件，不经过 provider：每次读、写、删都先拿进程内的监视器，再拿 `cloud-clipboard-keystore.lock` 上的文件锁（FileChannel 的锁按进程算，同一进程里两个线程靠监视器互斥），写入先写临时文件、落盘后原子改名。不用 SharedPreferences：它在每个进程里各缓存一份，另一个进程写的新值看不到。
 *
 * <p>一个安装只有一个设备身份。文件里已经有一个解得开的身份时，写入另一个身份一律拒绝（{@link CloudClipboardE2e.IdentityConflict}），后到的一方改用文件里的身份重来（见 {@link CloudClipboardE2e#run}）。身份只在两种情况下更换：退出登录时 {@link #delete}，或者文件解不开（AndroidKeyStore 里的密钥没了、文件被改过），那时旧身份本来就找不回来了。
 *
 * <p>自建中继（{@link CloudClipboardRelay}）的密钥串用同一套做法另存一份（{@link #forRelay}）：`noBackupFilesDir/cloud-clipboard-relay/cloud-clipboard-relay-keystore`，AndroidKeyStore 里另一把密钥。那份带着中继的绑定（{@link app.msime.android.clipboard.CloudClipboardRelayPolicy#binding}，地址和令牌的摘要，随密钥串一起加密）：绑定不同的内容当作没有，于是换了中继地址或令牌就会生成新的设备身份，写入时直接覆盖旧中继的那份。
 */
public final class CloudClipboardKeystore implements CloudClipboardE2e.Keystore {
    public static final String FILE_NAME = "cloud-clipboard-keystore";
    static final String LOCK_NAME = FILE_NAME + ".lock";
    /** 中继密钥串的文件名，与共享层 `clipboard_relay::RELAY_KEYSTORE_FILE` 同名：这个改动之前的开发构建由 Rust 在同一位置写过明文，第一次保存时被这里加密的那份原子替换。 */
    public static final String RELAY_FILE_NAME = "cloud-clipboard-relay-keystore";
    /** 驱动的密钥串最长约 125 个字符（`v1.` + 私钥 + 密钥编号 + 密钥），留出余量。 */
    static final int MAX_VALUE_BYTES = 1024;
    private static final int MAX_FILE_BYTES = 4096;
    private static final byte FORMAT_VERSION = 1;
    private static final String PREFIX = "v1.";
    private static final int SECRET_CHARS = 43;
    private static final Object LOCK = new Object();

    /** 加密与解密；Android 上是 AndroidKeyStore，JVM 冒烟里换成可逆的假实现。 */
    public interface Sealer {
        byte[] seal(byte[] plaintext) throws Exception;

        /** 解不开（密钥不在了、密文被改过）时抛 {@link Unreadable}；其他异常照常抛出，不当作身份丢失。 */
        byte[] open(byte[] sealed) throws Exception;
    }

    /** 文件在，但里面的身份已经找不回来。 */
    public static final class Unreadable extends Exception {
        private static final long serialVersionUID = 1L;

        public Unreadable() {
            super("cloud clipboard keystore unreadable");
        }
    }

    private final Path file;
    private final Path lockFile;
    private final Sealer sealer;
    /** 账号的那份为空串；中继的那份是中继的绑定，写在密文里密钥串前面，读出时不同就当作没有。 */
    private final String scope;

    public CloudClipboardKeystore(Path directory, Sealer sealer) {
        this(directory, FILE_NAME, "", sealer);
    }

    private CloudClipboardKeystore(Path directory, String fileName, String scope, Sealer sealer) {
        if (directory == null || sealer == null || scope == null || scope.indexOf('\n') >= 0)
            throw new IllegalArgumentException("keystore");
        this.file = directory.resolve(fileName);
        this.lockFile = directory.resolve(fileName + ".lock");
        this.sealer = sealer;
        this.scope = scope;
    }

    /**
     * 自建中继的密钥串：`directory` 下的 {@link #RELAY_FILE_NAME}，只认 `binding` 这个中继（{@link app.msime.android.clipboard.CloudClipboardRelayPolicy#binding}）的内容。目录到第一次读写时才建。
     */
    public static CloudClipboardKeystore forRelay(Path directory, String binding, Sealer sealer) {
        if (binding == null || binding.isEmpty()) throw new IllegalArgumentException("relay binding");
        return new CloudClipboardKeystore(directory, RELAY_FILE_NAME, binding, sealer);
    }

    /** 中继密钥串用的 AndroidKeyStore 密钥，与账号那份的密钥分开。 */
    public static Sealer relaySealer(Context context) {
        return new AndroidKeystoreSealer(context.getApplicationContext().getPackageName()
            + ".cloud-clipboard-relay-keystore.key");
    }

    /** 本应用私有目录里的那一份，主进程和 `:ime` 进程拿到的是同一个文件。 */
    public static CloudClipboardKeystore forContext(Context context) {
        Context application = context.getApplicationContext();
        File files = application.getFilesDir();
        if (files == null) throw new IllegalStateException("private files unavailable");
        return new CloudClipboardKeystore(files.toPath(),
            new AndroidKeystoreSealer(application.getPackageName() + ".cloud-clipboard-keystore.key"));
    }

    /**
     * 同一份文件，但到第一次读写时才去取私有目录：{@link BackendAccount} 在主线程上也会被构造，`getFilesDir` 可能碰盘，不能放在构造里。
     */
    public static CloudClipboardE2e.Keystore deferred(Context context) {
        Context application = context.getApplicationContext();
        return new CloudClipboardE2e.Keystore() {
            @Override public String load() throws Exception { return forContext(application).load(); }

            @Override public void save(String value, CloudClipboardE2e.Guard guard) throws Exception {
                forContext(application).save(value, guard);
            }

            @Override public void delete() throws Exception { forContext(application).delete(); }
        };
    }

    @Override public String load() throws Exception {
        synchronized (LOCK) {
            try (FileChannel channel = lockChannel()) {
                FileLock lock = channel.lock();
                try {
                    return read();
                } finally {
                    lock.release();
                }
            }
        }
    }

    @Override public void save(String value, CloudClipboardE2e.Guard guard) throws Exception {
        String identity = identity(value);
        if (identity == null || TextPolicy.utf8Length(value) > MAX_VALUE_BYTES)
            throw new IllegalArgumentException("invalid cloud clipboard keystore");
        synchronized (LOCK) {
            try (FileChannel channel = lockChannel()) {
                FileLock lock = channel.lock();
                try {
                    String current = read();
                    if (current != null && !identity.equals(identity(current)))
                        throw new CloudClipboardE2e.IdentityConflict(current);
                    if (guard != null) guard.check();
                    if (!value.equals(current)) write(value);
                } finally {
                    lock.release();
                }
            }
        }
    }

    @Override public void delete() throws IOException {
        synchronized (LOCK) {
            try (FileChannel channel = lockChannel()) {
                FileLock lock = channel.lock();
                try {
                    Files.deleteIfExists(file);
                } finally {
                    lock.release();
                }
            }
        }
    }

    /** 密钥串里代表设备身份的那一段（`v1.<私钥>`）；格式不对时为 null。 */
    static String identity(String value) {
        if (value == null || !value.startsWith(PREFIX) || value.length() > MAX_VALUE_BYTES) return null;
        int end = value.indexOf('.', PREFIX.length());
        String secret = value.substring(PREFIX.length(), end < 0 ? value.length() : end);
        if (secret.length() != SECRET_CHARS) return null;
        for (int index = 0; index < value.length(); index++) {
            char c = value.charAt(index);
            if (!(TextPolicy.isAsciiLetterOrDigit(c) || c == '-' || c == '_' || c == '.')) return null;
        }
        return PREFIX + secret;
    }

    private FileChannel lockChannel() throws IOException {
        Path parent = lockFile.getParent();
        // 中继的那份放在自己的子目录里，第一次用到时才建，只有本应用能进。
        if (parent != null && !Files.isDirectory(parent, LinkOption.NOFOLLOW_LINKS))
            Files.createDirectories(parent, PosixFilePermissions.asFileAttribute(PosixFilePermissions.fromString("rwx------")));
        return FileChannel.open(lockFile, StandardOpenOption.READ, StandardOpenOption.WRITE,
            StandardOpenOption.CREATE, LinkOption.NOFOLLOW_LINKS);
    }

    /** 在锁里读出并解密；没有文件、解不开或内容不是密钥串时为 null。 */
    private String read() throws Exception {
        byte[] sealed;
        try (FileChannel channel = FileChannel.open(file, StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)) {
            long size = channel.size();
            if (size <= 1 || size > MAX_FILE_BYTES) return null;
            ByteBuffer buffer = ByteBuffer.allocate((int) size);
            while (buffer.hasRemaining()) {
                if (channel.read(buffer) < 0) return null;
            }
            sealed = buffer.array();
        } catch (NoSuchFileException absent) {
            return null;
        }
        if (sealed[0] != FORMAT_VERSION) return null;
        byte[] plaintext;
        try {
            plaintext = sealer.open(Arrays.copyOfRange(sealed, 1, sealed.length));
        } catch (Unreadable lost) {
            return null;
        }
        String value = TextPolicy.utf8(plaintext);
        if (!scope.isEmpty()) {
            // 别的中继（或没有绑定的旧格式）写的：当作没有，保存时覆盖。
            if (!value.startsWith(scope + "\n")) return null;
            value = value.substring(scope.length() + 1);
        }
        return identity(value) == null ? null : value;
    }

    private void write(String value) throws Exception {
        byte[] sealed = sealer.seal(TextPolicy.utf8Bytes(scope.isEmpty() ? value : scope + "\n" + value));
        byte[] content = new byte[sealed.length + 1];
        content[0] = FORMAT_VERSION;
        System.arraycopy(sealed, 0, content, 1, sealed.length);
        Path parent = file.getParent();
        if (parent == null) throw new IOException("keystore directory unavailable");
        Path temporary = Files.createTempFile(parent, FILE_NAME + ".", ".tmp");
        try {
            try (FileChannel channel = FileChannel.open(temporary, StandardOpenOption.WRITE,
                    LinkOption.NOFOLLOW_LINKS)) {
                ByteBuffer buffer = ByteBuffer.wrap(content);
                while (buffer.hasRemaining()) {
                    if (channel.write(buffer) <= 0) throw new IOException("keystore write made no progress");
                }
                channel.force(true);
            }
            Files.move(temporary, file, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    /** AndroidKeyStore 里一把不导出的 AES-256-GCM 密钥，做法与 {@link AndroidAccountSessionStorage} 相同；输出是 12 字节 IV 加密文。 */
    static final class AndroidKeystoreSealer implements Sealer {
        private static final int IV_BYTES = 12;
        private static final int GCM_TAG_BITS = 128;
        private final String alias;

        AndroidKeystoreSealer(String alias) {
            this.alias = alias;
        }

        @Override public byte[] seal(byte[] plaintext) throws Exception {
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.ENCRYPT_MODE, loadOrCreateKey());
            byte[] iv = cipher.getIV();
            if (iv == null || iv.length != IV_BYTES) throw new IllegalStateException("secure_storage");
            byte[] ciphertext = cipher.doFinal(plaintext);
            byte[] sealed = new byte[IV_BYTES + ciphertext.length];
            System.arraycopy(iv, 0, sealed, 0, IV_BYTES);
            System.arraycopy(ciphertext, 0, sealed, IV_BYTES, ciphertext.length);
            return sealed;
        }

        @Override public byte[] open(byte[] sealed) throws Exception {
            if (sealed.length <= IV_BYTES + GCM_TAG_BITS / 8) throw new Unreadable();
            KeyStore store = KeyStore.getInstance("AndroidKeyStore");
            store.load(null);
            java.security.Key key = store.getKey(alias, null);
            if (!(key instanceof SecretKey secret)) throw new Unreadable();
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.DECRYPT_MODE, secret,
                new GCMParameterSpec(GCM_TAG_BITS, Arrays.copyOfRange(sealed, 0, IV_BYTES)));
            try {
                return cipher.doFinal(sealed, IV_BYTES, sealed.length - IV_BYTES);
            } catch (AEADBadTagException tampered) {
                throw new Unreadable();
            }
        }

        private SecretKey loadOrCreateKey() throws Exception {
            KeyStore store = KeyStore.getInstance("AndroidKeyStore");
            store.load(null);
            java.security.Key existing = store.getKey(alias, null);
            if (existing instanceof SecretKey secret) return secret;
            KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
            generator.init(new KeyGenParameterSpec.Builder(
                alias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .setRandomizedEncryptionRequired(true)
                .build());
            return generator.generateKey();
        }
    }
}
