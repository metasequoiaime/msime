package app.msime.android.policy;

import java.time.Instant;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;

/**
 * 「我的词条」经 WebDAV 网盘同步（`docs/dictionary-webdav-sync.md`）在 Android 宿主这一侧的纯逻辑：本机目录名、触发名、键盘进程什么时候可以自己跑同步、下一次定时同步隔多久，以及共享层错误码给人看的说法。不依赖 Android 和 org.json，JVM 冒烟直接测它。
 *
 * <p>同步本身（要不要联网、额度、防抖、退避、合并）全在 Rust 里（`msime_client_dictionary_sync`）；这里只把 Rust 给的结果翻译成界面和定时器要的东西，不复制那些规则。网盘地址、账号和应用密码的格式规则与云剪贴板的 WebDAV 相同，直接用 {@link app.msime.android.clipboard.CloudClipboardRelayPolicy} 的那几个检查。
 */
public final class DictionarySyncPolicy {
    /** 偏好里的键：`{url, username, password}`，没配置时不写进文档。与 `cloud_clipboard_webdav` 各配各的。 */
    public static final String PREFERENCE = "dictionary_sync_webdav";
    /** 本机同步状态和词库密钥串所在的目录名，在 noBackupFilesDir 下面；设置应用和 `:ime` 键盘进程给 Rust 的是同一个目录，否则同一台设备会登记成两台。 */
    public static final String DIRECTORY = "dictionary-sync";

    /** 用户改词之后：只记录本机改动，不联网。 */
    public static final String TRIGGER_LOCAL_CHANGE = "local_change";
    /** 打开设置里的词库同步页。 */
    public static final String TRIGGER_OPENED = "opened";
    /** 按上一次结果的 `next_check_ms` 定时再调。 */
    public static final String TRIGGER_PERIODIC = "periodic";
    /** 用户点「立即同步」。 */
    public static final String TRIGGER_MANUAL = "manual";

    public static final String CODE_NOT_CONFIGURED = "dictionary_sync_not_configured";
    public static final String CODE_INVALID = "invalid_dictionary_sync";
    public static final String CODE_UNAVAILABLE = "dictionary_sync_unavailable";
    public static final String CODE_UNAUTHORIZED = "dictionary_sync_unauthorized";
    public static final String CODE_RATE_LIMITED = "dictionary_sync_rate_limited";
    public static final String CODE_TOO_LARGE = "dictionary_sync_too_large";
    public static final String CODE_ROLLBACK = "dictionary_sync_rollback";
    public static final String CODE_UNSUPPORTED = "dictionary_sync_unsupported";
    public static final String CODE_CORRUPT = "dictionary_sync_corrupt";
    public static final String CODE_STORAGE = "dictionary_sync_storage";

    /**
     * 词库同步自己的错误码，与 `crates/client-core/src/cloud/dictionary_sync/sync.rs` 的 `SyncError::code` 加上 `crates/client-services/src/dictionary_webdav.rs` 的 `DictionarySyncError::code` 相同（`scripts/test-android-dictionary-sync-contract.py` 核对两边一致）。密钥串驱动给的码（`clipboard_device_pending`、`not_found` 等）不在这里，见 {@link #failureMessage}。
     */
    // 不用 List.of：它要 API 30，minSdk 是 28。
    public static final List<String> CODES = Arrays.asList(
        CODE_NOT_CONFIGURED, CODE_INVALID, CODE_UNAVAILABLE, CODE_UNAUTHORIZED, CODE_RATE_LIMITED,
        CODE_TOO_LARGE, CODE_ROLLBACK, CODE_UNSUPPORTED, CODE_CORRUPT, CODE_STORAGE);

    /** Rust 给的 `next_check_ms` 不合理（缺失、负数、超大）时的间隔：与核心的 `PULL_INTERVAL_MS` 相同，30 分钟。 */
    public static final long DEFAULT_DELAY_MILLIS = 30L * 60 * 1000;
    /** 定时器最短的间隔：Rust 给的最短重试是 5 秒，这里只防一个 0 让键盘进程空转。 */
    public static final long MIN_DELAY_MILLIS = 1_000;
    /** 定时器最长的间隔：宿主活着时每 30 分钟至少该拉一次，给一小时的余量。 */
    public static final long MAX_DELAY_MILLIS = 60L * 60 * 1000;
    /** 键盘进程在没配置、或者设置应用还没登记本机时，隔多久再看一次偏好（只读文件、不联网）。 */
    public static final long IDLE_RECHECK_MILLIS = 60_000;
    /** 键盘进程创建后等多久才第一次看：不和第一次建输入会话抢着读词库。 */
    public static final long START_DELAY_MILLIS = 10_000;
    /** 还有远端改动没写进本机时，键盘每次收起都趁没有输入会话再写一次，但两次至少隔这么久（与 Rust 第一次重试的间隔相同）。 */
    public static final long APPLY_RETRY_MILLIS = 5_000;

    private DictionarySyncPolicy() {}

    /** 下一次定时同步隔多久：Rust 的 `next_check_ms` 夹在 1 秒到 1 小时之间，读不到时按 30 分钟。 */
    public static long nextDelayMillis(Long nextCheckMs) {
        if (nextCheckMs == null || nextCheckMs < 0) return DEFAULT_DELAY_MILLIS;
        return Math.max(MIN_DELAY_MILLIS, Math.min(MAX_DELAY_MILLIS, nextCheckMs));
    }

    /**
     * 键盘进程里还没开始跑的那一次同步又被要了一次：两次合成一次。`periodic` 也会先记录本机改动，所以有它就是它；只有两次都是 `local_change` 时还是 `local_change`。键盘进程只用这两个触发，`opened` 和 `manual` 属于设置页。
     */
    public static String mergeImeTrigger(String queued, String next) {
        if (TRIGGER_PERIODIC.equals(queued) || TRIGGER_PERIODIC.equals(next)) return TRIGGER_PERIODIC;
        return TRIGGER_LOCAL_CHANGE;
    }

    /**
     * 键盘进程可不可以自己跑同步：只在本机已经有词库密钥串里的设备编号（`status.device_id`，32 位小写十六进制）时才跑。设备身份只由设置应用建：用户在设置里保存网盘之后，那一页的 `opened` 同步登记本机；键盘进程从不去做「第一次登记」，所以两个进程不会各自建一个身份，即使 Rust 的锁之外还有别的路径也一样。
     */
    public static boolean imeMayRun(String deviceId) {
        if (deviceId == null || deviceId.length() != 32) return false;
        for (int index = 0; index < deviceId.length(); index++) {
            char c = deviceId.charAt(index);
            if (!((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'))) return false;
        }
        return true;
    }

    /** 这一次同步失败的原因是不是连网盘本身就不行（连不上、凭据不对、限流或没配置）：是的话设置页不再接着读设备列表，免得用户多等一轮超时、多花一轮请求。 */
    public static boolean unreachable(String code) {
        return CODE_UNAVAILABLE.equals(code) || CODE_UNAUTHORIZED.equals(code)
            || CODE_RATE_LIMITED.equals(code) || CODE_NOT_CONFIGURED.equals(code);
    }

    /** 被挡下的大批删除的说明；没有被挡下的删除时为 null。 */
    public static String heldDeletionsMessage(Integer held) {
        if (held == null || held <= 0) return null;
        return "这台设备一次少了 " + held + " 个词，多半是本机词库被重置或损坏了，删除先没有同步出去，免得其他设备上的词也被删掉。确认是你自己删的，选「同步删除」；否则选「从网盘恢复」把这些词写回本机。";
    }

    /**
     * 自动同步在等额度时状态卡里的一句：`zone` 时区里的「HH:MM」，不是今天时写成「M月D日 HH:MM」；已经过了（不晚于 `nowMs`）或者没在等时为 null。与共享设置页的 `dictionarySyncBudgetWaitMessage`、iOS 和 Windows 逐字相同。
     */
    public static String budgetWaitMessage(Long untilMs, long nowMs, ZoneId zone) {
        if (untilMs == null || untilMs <= nowMs) return null;
        ZonedDateTime until = Instant.ofEpochMilli(untilMs).atZone(zone);
        String time = String.format(Locale.ROOT, "%02d:%02d", until.getHour(), until.getMinute());
        boolean today = until.toLocalDate().equals(Instant.ofEpochMilli(nowMs).atZone(zone).toLocalDate());
        String when = today ? time : until.getMonthValue() + "月" + until.getDayOfMonth() + "日 " + time;
        return "额度用完，将于 " + when + " 继续自动同步；可点「立即同步」。";
    }

    /** 状态行里「待发送 / 待写入」那一段；都为 0 时为空串。 */
    public static String pendingSummary(int upload, int apply) {
        StringBuilder out = new StringBuilder();
        if (upload > 0) out.append(upload).append(" 个改动待发送");
        if (apply > 0) {
            if (out.length() > 0) out.append(" · ");
            out.append(apply).append(" 个改动待写入本机");
        }
        return out.toString();
    }

    /**
     * 失败时给人看的一句话，与 iOS 的 `DictionarySyncPolicy.failureMessage` 和共享设置页的 `dictionarySyncErrorMessage` 逐字相同。`probe` 为真时说的是「测试连接」：`dictionary_sync_unsupported` 在那里是网盘不支持需要的文件操作，同步时是网盘上的格式比本机新。认不出的码给通用的一句，不抛出。
     */
    public static String failureMessage(String code, boolean probe) {
        String fallback = probe ? "测试连接失败，请稍后再试。" : "同步失败，请稍后再试。";
        if (code == null || code.isEmpty()) return fallback;
        return switch (code) {
            case CODE_NOT_CONFIGURED -> "地址、账号或应用密码不完整。";
            case CODE_INVALID -> "请求无效，请关闭设置后重新打开再试。";
            case CODE_UNAUTHORIZED -> "网盘拒绝了这个账号或应用密码。";
            case CODE_UNAVAILABLE -> "连不上网盘，请检查地址和网络；证书须由公网信任的机构签发。";
            case CODE_RATE_LIMITED -> "网盘暂时限制了请求次数，或本机这 30 分钟的同步次数已用完，请稍后再试。";
            case CODE_TOO_LARGE -> "网盘上的词库或本机的同步记录超过了上限，删减一些词条后再试。";
            case CODE_ROLLBACK -> "网盘上的词库比本机读到过的旧，或者被删改过。若是你自己清理了网盘文件，请用「重新信任网盘现状」单独确认恢复；测试连接不会清除防回退记录。";
            case CODE_UNSUPPORTED -> probe
                ? "这个网盘不支持词库同步需要的文件操作（独占创建、列目录），请换一个 WebDAV 服务或文件夹。"
                : "网盘上的词库是更新版本的格式，请先更新水杉输入法。";
            case CODE_CORRUPT -> "网盘上的词库记录损坏了，自动同步已暂停。";
            case CODE_STORAGE -> "本机的同步记录读写失败。";
            case "clipboard_device_pending" -> "本设备还在等待允许。";
            case "not_found" -> "这台设备已经不在列表里了，请刷新后再试。";
            case "clipboard_key_conflict", "clipboard_keystore_conflict" -> "另一台设备同时修改了密钥，请稍后再试。";
            case "clipboard_device_limit" -> "设备数已达上限，请先移除不再使用的设备。";
            default -> fallback;
        };
    }
}
