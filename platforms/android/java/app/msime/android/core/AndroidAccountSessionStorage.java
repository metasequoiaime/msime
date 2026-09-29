package app.msime.android;

import android.content.Context;
import android.content.SharedPreferences;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.Base64;
import java.nio.charset.StandardCharsets;
import java.security.KeyStore;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/** Device-bound encrypted storage for the backend account session. */
public final class AndroidAccountSessionStorage implements BackendAccount.SessionStore {
    public static final String PREFERENCES_NAME = "msime_account_session_v1";
    public static final int MAX_PAYLOAD_BYTES = 16 * 1024;
    private static final String KEY_IV = "iv";
    private static final String KEY_CIPHERTEXT = "ciphertext";
    private static final int IV_BYTES = 12;
    private static final int GCM_TAG_BITS = 128;
    private final SharedPreferences preferences;
    private final String keyAlias;

    public AndroidAccountSessionStorage(Context context) {
        this(context, PREFERENCES_NAME);
    }

    public AndroidAccountSessionStorage(Context context, String preferencesName) {
        Context application = context.getApplicationContext();
        if (preferencesName == null || !preferencesName.matches("[a-zA-Z0-9_.-]{1,80}")) {
            throw new IllegalArgumentException("secure_storage");
        }
        preferences = application.getSharedPreferences(preferencesName, Context.MODE_PRIVATE);
        keyAlias = preferencesName.equals(PREFERENCES_NAME)
            ? application.getPackageName() + ".account.session.v1"
            : application.getPackageName() + "." + preferencesName + ".key";
    }

    public synchronized String load() throws Exception {
        String encodedIv = preferences.getString(KEY_IV, null);
        String encodedCiphertext = preferences.getString(KEY_CIPHERTEXT, null);
        if (encodedIv == null && encodedCiphertext == null) return null;
        if (encodedIv == null || encodedCiphertext == null) throw new IllegalStateException("secure_storage");
        byte[] iv = Base64.decode(encodedIv, Base64.NO_WRAP);
        byte[] ciphertext = Base64.decode(encodedCiphertext, Base64.NO_WRAP);
        if (iv.length != IV_BYTES || ciphertext.length <= GCM_TAG_BITS / 8
                || ciphertext.length > MAX_PAYLOAD_BYTES + GCM_TAG_BITS / 8)
            throw new IllegalStateException("secure_storage");
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.DECRYPT_MODE, loadKey(), new GCMParameterSpec(GCM_TAG_BITS, iv));
        byte[] plaintext = cipher.doFinal(ciphertext);
        if (plaintext.length == 0 || plaintext.length > MAX_PAYLOAD_BYTES)
            throw new IllegalStateException("secure_storage");
        return new String(plaintext, StandardCharsets.UTF_8);
    }

    public synchronized void save(String value) throws Exception {
        if (value == null) throw new IllegalArgumentException("secure_storage");
        byte[] plaintext = value.getBytes(StandardCharsets.UTF_8);
        if (plaintext.length == 0 || plaintext.length > MAX_PAYLOAD_BYTES)
            throw new IllegalArgumentException("secure_storage");
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.ENCRYPT_MODE, loadOrCreateKey());
        byte[] iv = cipher.getIV();
        if (iv == null || iv.length != IV_BYTES) throw new IllegalStateException("secure_storage");
        byte[] ciphertext = cipher.doFinal(plaintext);
        boolean committed = preferences.edit()
            .putString(KEY_IV, Base64.encodeToString(iv, Base64.NO_WRAP))
            .putString(KEY_CIPHERTEXT, Base64.encodeToString(ciphertext, Base64.NO_WRAP))
            .commit();
        if (!committed) throw new IllegalStateException("secure_storage");
    }

    public synchronized void clear() throws Exception {
        if (!preferences.edit().clear().commit()) throw new IllegalStateException("secure_storage");
    }

    private SecretKey loadKey() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        java.security.Key key = store.getKey(keyAlias, null);
        if (!(key instanceof SecretKey)) throw new IllegalStateException("secure_storage");
        return (SecretKey) key;
    }

    private SecretKey loadOrCreateKey() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        java.security.Key existing = store.getKey(keyAlias, null);
        if (existing instanceof SecretKey) return (SecretKey) existing;
        KeyGenerator generator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        generator.init(new KeyGenParameterSpec.Builder(
            keyAlias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setRandomizedEncryptionRequired(true)
            .build());
        return generator.generateKey();
    }
}
