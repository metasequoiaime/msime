package app.msime.android;

import android.content.Context;
import java.util.Objects;

/** 仅扩展学习密钥槽；原四项及其剪贴板、匿名身份锁策略由原存储持有。 */
final class AccountLearningSessionStore implements NativeClient.SessionStore, LearningKeystoreSupport {
    private static final int MAX_BYTES = 4096;
    private final NativeClient.SessionStore delegate;
    private final Storage learning;

    interface Storage {
        String load() throws Exception;
        void save(String value) throws Exception;
        void clear() throws Exception;
    }

    static AccountLearningSessionStore create(Context context, NativeClient.SessionStore delegate) {
        AndroidAccountSessionStorage storage = new AndroidAccountSessionStorage(context, "learning-keystore");
        return new AccountLearningSessionStore(delegate, new Storage() {
            @Override public String load() throws Exception { return storage.load(); }
            @Override public void save(String value) throws Exception { storage.save(value); }
            @Override public void clear() throws Exception { storage.clear(); }
        });
    }

    AccountLearningSessionStore(NativeClient.SessionStore delegate, Storage learning) {
        this.delegate = Objects.requireNonNull(delegate);
        this.learning = Objects.requireNonNull(learning);
    }

    @Override public boolean supportsLearningKeystore() { return true; }

    @Override public byte[] load(int item) {
        if (item != ITEM_LEARNING_KEYSTORE) return delegate.load(item);
        try {
            String value = learning.load();
            if (value == null) return null;
            byte[] bytes = TextPolicy.utf8StrictBytes(value);
            validate(bytes);
            return bytes;
        } catch (Exception error) {
            throw new IllegalStateException("learning_storage", error);
        }
    }

    @Override public int save(int item, byte[] bytes) {
        if (item != ITEM_LEARNING_KEYSTORE) return delegate.save(item, bytes);
        try {
            validate(bytes);
            learning.save(TextPolicy.utf8Strict(bytes));
            return 0;
        } catch (Exception error) {
            throw new IllegalStateException("learning_storage", error);
        }
    }

    @Override public int remove(int item) {
        if (item != ITEM_LEARNING_KEYSTORE) return delegate.remove(item);
        try {
            learning.clear();
            return 0;
        } catch (Exception error) {
            throw new IllegalStateException("learning_storage", error);
        }
    }

    private static void validate(byte[] bytes) {
        if (bytes == null || bytes.length == 0 || bytes.length > MAX_BYTES)
            throw new IllegalArgumentException("learning_storage");
    }
}
