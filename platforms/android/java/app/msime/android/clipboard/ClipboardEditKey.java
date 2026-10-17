package app.msime.android;

import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import java.security.KeyStore;
import javax.crypto.Mac;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;

/** 为剪贴板编辑页生成安装级、不可伪造的深链令牌。 */
public final class ClipboardEditKey {
    private static final String KEYSTORE = "AndroidKeyStore";
    private static final String ALIAS_SUFFIX = ".clipboard.edit.v1";
    private static final String HMAC = "HmacSHA256";
    private static final int TAG_HEX_LENGTH = 64;

    private ClipboardEditKey() {}

    /** 用 AndroidKeyStore 中本安装的密钥生成令牌；密钥不可用时安全失败，不回退到可预测算法。 */
    public static String editKey(Context context, long timestamp, String text) {
        if (context == null || text == null) throw new IllegalArgumentException("clipboard edit key");
        try {
            Context application = context.getApplicationContext();
            SecretKey key = loadOrCreateKey(application.getPackageName() + ALIAS_SUFFIX);
            return editKey(key, timestamp, text);
        } catch (Exception | LinkageError unavailable) {
            return null;
        }
    }

    /** 由调用方提供密钥，供 JVM 冒烟验证 HMAC 输入；生产代码使用上面的 AndroidKeyStore 入口。 */
    public static String editKey(SecretKey key, long timestamp, String text) {
        if (key == null || text == null) throw new IllegalArgumentException("clipboard edit key");
        try {
            Mac mac = Mac.getInstance(HMAC);
            mac.init(key);
            String input = BoundsPolicy.nonNegative(timestamp) + "\u0000" + text.length() + "\u0000" + text;
            byte[] tag = mac.doFinal(TextPolicy.utf8Bytes(input));
            return format(timestamp, hex(tag), text.length());
        } catch (Exception error) {
            throw new IllegalStateException("clipboard edit key", error);
        }
    }

    /** 令牌的纯格式部分；深链只传时间戳、固定长度标签和文字长度。 */
    public static String format(long timestamp, String tag, int textLength) {
        long normalizedTimestamp = BoundsPolicy.nonNegative(timestamp);
        if (tag == null || tag.length() != TAG_HEX_LENGTH || textLength < 0) {
            throw new IllegalArgumentException("clipboard edit key");
        }
        for (int index = 0; index < tag.length(); index++) {
            char value = tag.charAt(index);
            if (!((value >= '0' && value <= '9') || (value >= 'a' && value <= 'f'))) {
                throw new IllegalArgumentException("clipboard edit key");
            }
        }
        return normalizedTimestamp + ":" + tag + ":" + textLength;
    }

    private static String hex(byte[] bytes) {
        char[] digits = "0123456789abcdef".toCharArray();
        char[] output = new char[bytes.length * 2];
        for (int index = 0; index < bytes.length; index++) {
            int value = bytes[index] & 0xff;
            output[index * 2] = digits[value >>> 4];
            output[index * 2 + 1] = digits[value & 0x0f];
        }
        return new String(output);
    }

    private static SecretKey loadOrCreateKey(String alias) throws Exception {
        KeyStore store = KeyStore.getInstance(KEYSTORE);
        store.load(null);
        java.security.Key existing = store.getKey(alias, null);
        if (existing instanceof SecretKey) return (SecretKey) existing;
        KeyGenerator generator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_HMAC_SHA256, KEYSTORE);
        generator.init(new KeyGenParameterSpec.Builder(
            alias, KeyProperties.PURPOSE_SIGN | KeyProperties.PURPOSE_VERIFY)
            .setDigests(KeyProperties.DIGEST_SHA256)
            .build());
        try {
            return generator.generateKey();
        } catch (Exception race) {
            store.load(null);
            java.security.Key concurrent = store.getKey(alias, null);
            if (concurrent instanceof SecretKey) return (SecretKey) concurrent;
            throw race;
        }
    }
}
