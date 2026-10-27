import app.msime.android.policy.DictionarySyncPolicy;
import java.util.HashSet;
import java.util.Set;

/**
 * 「我的词条」网盘同步在 Android 这一侧的纯逻辑：键盘进程只在本机已有设备编号时自己跑（不做第一次登记），键盘里排着的两次同步合成一次且不丢 `periodic`，Rust 给的下一次间隔夹在合理范围里，每个词库同步错误码都有自己的说法（不落到通用的那一句），「测试连接」里的 `dictionary_sync_unsupported` 说的是网盘不支持文件操作。
 */
public final class DictionarySyncPolicySmoke {
    private static final String GENERIC = DictionarySyncPolicy.failureMessage("synthetic_unknown_code", false);

    public static void main(String[] args) {
        // 第一次登记只由设置页做：键盘进程看到没有设备编号、或者编号不是 32 位小写十六进制时不跑。
        check(!DictionarySyncPolicy.imeMayRun(null), "no device id means the keyboard does not run");
        check(!DictionarySyncPolicy.imeMayRun(""), "an empty device id means the keyboard does not run");
        check(!DictionarySyncPolicy.imeMayRun("0123456789abcdef0123456789abcde"), "31 hex digits are not a device id");
        check(!DictionarySyncPolicy.imeMayRun("0123456789ABCDEF0123456789ABCDEF"), "upper-case hex is not a device id");
        check(!DictionarySyncPolicy.imeMayRun("0123456789abcdef0123456789abcdeg"), "a non-hex digit is not a device id");
        check(DictionarySyncPolicy.imeMayRun("0123456789abcdef0123456789abcdef"), "a registered device lets the keyboard run");

        // 键盘只用 local_change 和 periodic；periodic 也会先记录本机改动，所以合并后有 periodic 就是 periodic。
        String local = DictionarySyncPolicy.TRIGGER_LOCAL_CHANGE;
        String periodic = DictionarySyncPolicy.TRIGGER_PERIODIC;
        check(local.equals(DictionarySyncPolicy.mergeImeTrigger(local, local)), "two local changes stay one local change");
        check(periodic.equals(DictionarySyncPolicy.mergeImeTrigger(local, periodic)), "a later periodic absorbs a queued local change");
        check(periodic.equals(DictionarySyncPolicy.mergeImeTrigger(periodic, local)), "a queued periodic absorbs a later local change");
        check(periodic.equals(DictionarySyncPolicy.mergeImeTrigger(periodic, periodic)), "two periodic runs stay one");

        // next_check_ms：缺失或负数按 30 分钟，0 不让键盘空转，超大的夹到一小时。
        check(DictionarySyncPolicy.nextDelayMillis(null) == DictionarySyncPolicy.DEFAULT_DELAY_MILLIS, "a missing interval is 30 minutes");
        check(DictionarySyncPolicy.nextDelayMillis(-5L) == DictionarySyncPolicy.DEFAULT_DELAY_MILLIS, "a negative interval is 30 minutes");
        check(DictionarySyncPolicy.nextDelayMillis(0L) == DictionarySyncPolicy.MIN_DELAY_MILLIS, "a zero interval is clamped up");
        check(DictionarySyncPolicy.nextDelayMillis(5_000L) == 5_000L, "the 5 second deferred-write retry is kept");
        check(DictionarySyncPolicy.nextDelayMillis(60_000L) == 60_000L, "the one minute debounce is kept");
        check(DictionarySyncPolicy.nextDelayMillis(1_800_000L) == 1_800_000L, "the 30 minute pull is kept");
        check(DictionarySyncPolicy.nextDelayMillis(Long.MAX_VALUE) == DictionarySyncPolicy.MAX_DELAY_MILLIS, "a huge interval is clamped to an hour");

        // 每个词库同步码都有自己的说法，互不相同，也不是通用的那一句。
        Set<String> messages = new HashSet<>();
        for (String code : DictionarySyncPolicy.CODES) {
            String message = DictionarySyncPolicy.failureMessage(code, false);
            check(message != null && !message.isEmpty() && !message.equals(GENERIC), code + " has its own message");
            check(messages.add(message), code + " does not share another code's message");
        }
        check(DictionarySyncPolicy.CODES.size() == 10, "all ten dictionary sync codes are listed");
        // 密钥串驱动给的码也有自己的说法；不认识的码给通用的一句，不抛出。
        for (String code : new String[] {"clipboard_device_pending", "clipboard_key_conflict", "clipboard_keystore_conflict",
                "clipboard_device_limit", "not_found"}) {
            check(!DictionarySyncPolicy.failureMessage(code, false).equals(GENERIC), code + " has its own message");
        }
        check(GENERIC.equals(DictionarySyncPolicy.failureMessage(null, false)), "a missing code gets the generic message");
        check(GENERIC.equals(DictionarySyncPolicy.failureMessage("", true)), "an empty code gets the generic message");
        // 网盘拒绝凭据不是水杉账号没登录，说法不提登录。
        check(!DictionarySyncPolicy.failureMessage(DictionarySyncPolicy.CODE_UNAUTHORIZED, false).contains("登录"),
            "rejected WebDAV credentials do not ask the user to sign in");
        // 测试连接里「不认识的格式」是网盘不支持文件操作；同步时是网盘上的格式更新。
        String probeUnsupported = DictionarySyncPolicy.failureMessage(DictionarySyncPolicy.CODE_UNSUPPORTED, true);
        String syncUnsupported = DictionarySyncPolicy.failureMessage(DictionarySyncPolicy.CODE_UNSUPPORTED, false);
        check(probeUnsupported.contains("文件操作") && !syncUnsupported.equals(probeUnsupported),
            "unsupported reads differently in the probe and in a sync");
        // 回退的说法告诉用户怎么恢复：重新测试连接并保存。
        check(DictionarySyncPolicy.failureMessage(DictionarySyncPolicy.CODE_ROLLBACK, false).contains("重新测试连接并保存"),
            "rollback tells the user how to recover");

        // 连网盘本身就不行时，设置页不再接着读设备；其余失败（例如回退、等待允许）照样读设备。
        for (String code : new String[] {DictionarySyncPolicy.CODE_UNAVAILABLE, DictionarySyncPolicy.CODE_UNAUTHORIZED,
                DictionarySyncPolicy.CODE_RATE_LIMITED, DictionarySyncPolicy.CODE_NOT_CONFIGURED}) {
            check(DictionarySyncPolicy.unreachable(code), code + " skips the device read");
        }
        for (String code : new String[] {null, DictionarySyncPolicy.CODE_ROLLBACK, DictionarySyncPolicy.CODE_CORRUPT,
                "clipboard_device_pending"}) {
            check(!DictionarySyncPolicy.unreachable(code), code + " still reads the devices");
        }

        // 被挡下的删除：没有时没有说明，有时写出数量和两个选择。
        check(DictionarySyncPolicy.heldDeletionsMessage(null) == null, "no held deletions means no message");
        check(DictionarySyncPolicy.heldDeletionsMessage(0) == null, "zero held deletions means no message");
        String held = DictionarySyncPolicy.heldDeletionsMessage(73);
        check(held != null && held.contains("73") && held.contains("同步删除") && held.contains("从网盘恢复"),
            "held deletions name the count and both choices");

        check(DictionarySyncPolicy.pendingSummary(0, 0).isEmpty(), "nothing pending is an empty summary");
        check(DictionarySyncPolicy.pendingSummary(3, 0).equals("3 个改动待发送"), "pending uploads are counted");
        check(DictionarySyncPolicy.pendingSummary(0, 2).equals("2 个改动待写入本机"), "pending local writes are counted");
        check(DictionarySyncPolicy.pendingSummary(3, 2).equals("3 个改动待发送 · 2 个改动待写入本机"), "both are joined");

        // 目录与文档一致：两个进程共用 noBackupFilesDir 下的同一个子目录。
        check(DictionarySyncPolicy.DIRECTORY.equals("dictionary-sync"), "the shared directory is noBackupFilesDir/dictionary-sync");
        check(DictionarySyncPolicy.PREFERENCE.equals("dictionary_sync_webdav"), "the preference key matches Rust");
        System.out.println("DictionarySyncPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
