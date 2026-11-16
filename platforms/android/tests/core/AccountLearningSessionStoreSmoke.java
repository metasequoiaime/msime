package app.msime.android;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;

/** 学习密钥经产品适配器独立读写；加密存储用内存替身，不依赖 Android 系统。 */
public final class AccountLearningSessionStoreSmoke {
    private static final int LEARNING = NativeClient.SessionStore.ITEM_LEARNING_KEYSTORE;

    public static void main(String[] args) {
        Delegate delegate = new Delegate();
        Memory learning = new Memory();
        NativeClient.SessionStore store = new AccountLearningSessionStore(delegate, learning);
        check(!delegate.supportsLearningKeystore(), "旧存储默认不声明学习能力");
        check(store.supportsLearningKeystore(), "扩展存储声明学习能力");
        check(store.load(LEARNING) == null, "学习项初始不存在");
        for (int item = 1; item <= 4; item++) {
            byte[] bytes = ("synthetic-slot-" + item).getBytes(StandardCharsets.UTF_8);
            check(store.save(item, bytes) == 1, "保留原存储的保存返回值");
            check(store.load(item) == bytes, "原四项逐字节直接委托");
        }
        byte[] original = "合成学习密钥".getBytes(StandardCharsets.UTF_8);
        check(store.save(LEARNING, original) == 0, "学习项保存成功");
        original[0] = 0;
        check("合成学习密钥".equals(learning.value), "保存不保留可变输入数组");
        check(store.remove(3) == 7 && store.remove(4) == 7, "匿名清理保留原返回值");
        check(learning.value != null && delegate.items.size() == 2, "匿名清理不碰学习或登录域");
        check(store.remove(LEARNING) == 0 && store.load(LEARNING) == null, "学习项同步清除");
        check(delegate.items.size() == 2, "学习清理不碰账号或剪贴板");
        check(store.remove(LEARNING) == 0, "重复清理成功");

        byte[] boundary = ("界".repeat(1365) + "a").getBytes(StandardCharsets.UTF_8);
        check(boundary.length == 4096 && store.save(LEARNING, boundary) == 0, "以 UTF-8 字节计算 4 KiB 边界");
        check(Arrays.equals(boundary, store.load(LEARNING)), "完整读回边界数据");
        for (byte[] invalid : new byte[][]{null, new byte[0], new byte[4097], {(byte) 0xc3, 0x28}}) {
            fails(() -> store.save(LEARNING, invalid));
            check(Arrays.equals(boundary, store.load(LEARNING)), "非法保存保留旧数据");
        }
        for (String invalid : new String[]{"", "界".repeat(1366), "\ud800"}) {
            learning.value = invalid;
            fails(() -> store.load(LEARNING));
        }
        learning.failure = new IOException("合成存储失败");
        check(fails(() -> store.load(LEARNING)).getCause() == learning.failure, "读取失败不得冒充不存在");
        check(fails(() -> store.save(LEARNING, boundary)).getCause() == learning.failure, "保存失败不得报成功");
        check(fails(() -> store.remove(LEARNING)).getCause() == learning.failure, "清理失败不得报成功");
        delegate.failure = new IllegalArgumentException("合成原存储失败");
        check(fails(() -> store.load(1)) == delegate.failure, "保留原存储读取异常");
        check(fails(() -> store.save(2, boundary)) == delegate.failure, "保留原存储保存异常");
        check(fails(() -> store.remove(3)) == delegate.failure, "保留原存储清理异常");
        System.out.println("Android 学习密钥隔离、边界、失败传播和旧存储兼容通过");
    }

    private static RuntimeException fails(Runnable operation) {
        try { operation.run(); }
        catch (RuntimeException expected) { return expected; }
        throw new AssertionError("应当报告失败");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class Memory implements AccountLearningSessionStore.Storage {
        String value;
        IOException failure;
        @Override public String load() throws IOException { verify(); return value; }
        @Override public void save(String text) throws IOException { verify(); value = text; }
        @Override public void clear() throws IOException { verify(); value = null; }
        private void verify() throws IOException { if (failure != null) throw failure; }
    }

    private static final class Delegate implements NativeClient.SessionStore {
        final Map<Integer, byte[]> items = new HashMap<>();
        RuntimeException failure;
        @Override public byte[] load(int item) { verify(); return items.get(item); }
        @Override public int save(int item, byte[] bytes) { verify(); items.put(item, bytes); return 1; }
        @Override public int remove(int item) { verify(); items.remove(item); return 7; }
        private void verify() { if (failure != null) throw failure; }
    }
}
