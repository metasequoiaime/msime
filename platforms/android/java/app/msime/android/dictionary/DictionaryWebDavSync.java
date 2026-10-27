package app.msime.android;

import android.content.Context;
import android.os.Build;
import app.msime.android.policy.DictionarySyncPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「我的词条」经 WebDAV 网盘同步（`docs/dictionary-webdav-sync.md`）的 Java 这一层：把设置页和键盘进程要做的动作拼成 `msime_client_dictionary_sync` 的请求，把回答解析成记录。同步的规则（联网与否、额度、防抖、退避、合并、设备批准）都在 Rust 里，这里不复制。
 *
 * <p>本机目录是 `noBackupFilesDir/dictionary-sync`（{@link #directory}）：设置应用和 `:ime` 键盘进程给的是同一个目录，里面的锁让两个进程的同步一个接一个跑，词库密钥串（同一个目录下的 `cloud-clipboard-relay-keystore`）在锁里「读不到才生成」，所以一个安装只有一个设备身份。那份密钥串由 Rust 存成只有本应用能读的文件，不像云剪贴板中继那样由 Java 用 AndroidKeyStore 加密：这个入口没有让宿主保管密钥串的接口，noBackupFilesDir 也不进云备份和设备间迁移。键盘进程还多守一条：本机还没有设备编号时不跑（{@link DictionarySyncPolicy#imeMayRun}），第一次登记只由设置页做。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它。除 {@link #status} 外每个动作都阻塞在网络上，不要在主线程或输入线程上调用。
 */
public final class DictionaryWebDavSync {
    /** 一次 `status`：本机在词库密钥串里的编号和核对码（还没用过网盘时为 null）、是否已获准、共享词库里的词数、待发送和待写入的改动数、被挡下的大批删除（没有时为 null）、时钟是否不准、上次成功的时间（毫秒，没有时为 null）和上次的错误码（没有时为 null）。 */
    public record Status(String deviceId, String code, boolean approved, boolean joined, int words,
            int pendingUpload, int pendingApply, Integer heldDeletions, boolean clockSkew, Long lastSuccessMs,
            String lastError) {}

    /** 一次 `run`：这一轮有没有联网、下一次定时同步隔多久、同步后的状态；失败时 `error` 是错误码，`report` 为 null。 */
    public record Run(boolean network, Long nextCheckMs, Status status, String error, Report report) {}

    /** 一次成功同步的报告：记下的本机改动数、收到的事件数、写进本机的改动数、输入法正占着词库而没写成（`deferred`）、发出去的操作数、是不是还在等密钥。 */
    public record Report(int captured, int receivedEvents, int applied, boolean deferred, int publishedOps,
            boolean pendingKey) {}

    /** 共享层答了 `ok:false`，或者回答读不懂（`invalid_response`）。`code` 见 {@link DictionarySyncPolicy#failureMessage}。 */
    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        public final String code;

        public Failure(String code) {
            super(code);
            this.code = code;
        }
    }

    private final String options;
    private final String directory;
    private final String deviceName;

    private DictionaryWebDavSync(String options, String directory, String deviceName) {
        this.options = options;
        this.directory = directory;
        this.deviceName = deviceName;
    }

    /** 按本应用的 runtime-options.json 和 noBackupFilesDir 打开；首次设置之前（还没有 runtime-options.json）不可能配置过网盘，抛出 `dictionary_sync_not_configured`。读文件，要在工作线程上调用。 */
    public static DictionaryWebDavSync open(Context context) throws Failure {
        return open(HostOptionsPolicy.readRuntimeOptions(context.getFilesDir()), directory(context),
            CloudClipboardE2e.deviceName(Build.MANUFACTURER, Build.MODEL));
    }

    /** 同上，HostOptions、目录和设备名由调用方给出；设备冒烟（另一个包）用临时目录经它跑这一层。 */
    public static DictionaryWebDavSync open(String options, String directory, String deviceName) throws Failure {
        if (options == null || options.isEmpty()) throw new Failure(DictionarySyncPolicy.CODE_NOT_CONFIGURED);
        return new DictionaryWebDavSync(options, directory, deviceName);
    }

    /** 本机同步状态和词库密钥串的目录：`noBackupFilesDir/dictionary-sync`，由 Rust 第一次用到时建成只有本应用能进。 */
    public static String directory(Context context) {
        return new File(context.getNoBackupFilesDir(), DictionarySyncPolicy.DIRECTORY).getAbsolutePath();
    }

    /** 跑一次同步。`trigger` 是 {@link DictionarySyncPolicy} 的四个触发之一；`confirmDeletions`、`restoreDeletions` 是用户对被挡下的大批删除的选择。同步失败不抛出，答在 {@link Run#error} 里，状态照样给出。 */
    public Run run(String trigger, boolean confirmDeletions, boolean restoreDeletions) throws Failure {
        JSONObject action = action("run");
        try {
            action.put("trigger", trigger);
            if (confirmDeletions) action.put("confirm_deletions", true);
            if (restoreDeletions) action.put("restore_deletions", true);
        } catch (JSONException impossible) {
            throw new Failure(DictionarySyncPolicy.CODE_INVALID);
        }
        return parseRun(call(action));
    }

    /** 不联网的概况。 */
    public Status status() throws Failure {
        return parseStatus(call(action("status")));
    }

    /** 词库密钥串里的设备，形状与云剪贴板的设备管理相同；本机还没登记时登记（第一台设备生成密钥）。联网。 */
    public CloudClipboardApi.Devices devices() throws Failure {
        return parseDevices(call(action("devices")));
    }

    /** 允许一台等待中的设备。 */
    public CloudClipboardApi.Devices approve(String deviceId) throws Failure {
        return parseDevices(call(device("approve", deviceId)));
    }

    /** 拒绝一台等待中的设备。 */
    public CloudClipboardApi.Devices reject(String deviceId) throws Failure {
        return parseDevices(call(device("reject", deviceId)));
    }

    /** 移除一台设备并换钥（其他设备要重新允许），随后同步一次、用新密钥重写共享词库。 */
    public CloudClipboardApi.Devices remove(String deviceId) throws Failure {
        return parseDevices(member(call(device("remove", deviceId)), "devices"));
    }

    /** 换一把只给本机的新密钥（其他设备要重新允许），随后同步一次。 */
    public CloudClipboardApi.Devices reset() throws Failure {
        return parseDevices(member(call(action("reset")), "devices"));
    }

    /** 以本机为准重新开始：持有旧密钥的设备都不在了时由用户选，其他设备的词随后合并进来，不删词。 */
    public Status restart() throws Failure {
        return parseStatus(call(action("restart")));
    }

    /**
     * 「测试连接」：用设置页里还没保存的草稿试一遍网盘行为，不读偏好，也不改本机同步位置、待发改动或防回退记录。请求里有应用密码，不写日志。大约二十个请求，阻塞在网络上。
     */
    public void probe(String url, String username, String password) throws Failure {
        probe(url, username, password, false);
    }

    /** 只有用户单独确认恢复后才传 `trustRemote`，让本机忘掉同步位置和待发改动。 */
    public void probe(String url, String username, String password, boolean trustRemote) throws Failure {
        JSONObject action = action("probe");
        try {
            action.put("url", url).put("username", username).put("password", password);
            if (trustRemote) action.put("trust_remote", true);
        } catch (JSONException impossible) {
            throw new Failure(DictionarySyncPolicy.CODE_INVALID);
        }
        call(action);
    }

    private static JSONObject action(String operation) {
        try {
            return new JSONObject().put("operation", operation);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static JSONObject device(String operation, String deviceId) throws Failure {
        if (!CloudClipboardE2e.validDeviceId(deviceId)) throw new Failure(DictionarySyncPolicy.CODE_INVALID);
        try {
            return action(operation).put("device_id", deviceId);
        } catch (JSONException impossible) {
            throw new Failure(DictionarySyncPolicy.CODE_INVALID);
        }
    }

    /** 发一个请求，拆 `{"ok":true,"value":{…}}`；`ok` 不是真正的 true 时以 `error` 为错误码抛出。 */
    private JSONObject call(JSONObject action) throws Failure {
        String response;
        try {
            JSONObject request = new JSONObject()
                .put("options", new JSONObject(options))
                .put("directory", directory)
                .put("action", action);
            if (deviceName != null && !deviceName.isEmpty()) request.put("device_name", deviceName);
            response = NativeClient.dictionarySync(request.toString());
        } catch (JSONException | IllegalArgumentException malformed) {
            throw new Failure(DictionarySyncPolicy.CODE_INVALID);
        } catch (RuntimeException | LinkageError unavailable) {
            // 共享库没加载上；原因不带出去。
            throw new Failure("invalid_response");
        }
        try {
            JSONObject root = new JSONObject(response);
            if (!JsonPolicy.strictTrue(root.opt("ok"))) {
                String code = JsonPolicy.strictString(root.opt("error"));
                throw new Failure(code == null || code.isEmpty() ? "invalid_response" : code);
            }
            JSONObject value = root.optJSONObject("value");
            if (value == null) throw new Failure("invalid_response");
            return value;
        } catch (JSONException malformed) {
            throw new Failure("invalid_response");
        }
    }

    private static JSONObject member(JSONObject value, String key) throws Failure {
        JSONObject member = value.optJSONObject(key);
        if (member == null) throw new Failure("invalid_response");
        return member;
    }

    private static CloudClipboardApi.Devices parseDevices(JSONObject value) throws Failure {
        try {
            return CloudClipboardApi.parseDevices(value.toString());
        } catch (CloudApi.Failure invalid) {
            throw new Failure("invalid_response");
        }
    }

    static Run parseRun(JSONObject value) throws Failure {
        Boolean network = JsonPolicy.strictBoolean(value.opt("network"));
        JSONObject status = value.optJSONObject("status");
        if (network == null || status == null) throw new Failure("invalid_response");
        Long next = JsonPolicy.nonNegativeLong(value.opt("next_check_ms"));
        JSONObject rawReport = value.optJSONObject("report");
        String error = optionalString(value.opt("error"));
        Report report = null;
        if (rawReport != null) {
            Boolean deferred = JsonPolicy.strictBoolean(rawReport.opt("deferred"));
            Boolean pendingKey = JsonPolicy.strictBoolean(rawReport.opt("pending_key"));
            if (deferred == null || pendingKey == null) throw new Failure("invalid_response");
            report = new Report(count(rawReport.opt("captured")), count(rawReport.opt("received_events")),
                count(rawReport.opt("applied")), deferred, count(rawReport.opt("published_ops")), pendingKey);
        } else if (error == null) {
            throw new Failure("invalid_response");
        }
        return new Run(network, next, parseStatus(status), error, report);
    }

    static Status parseStatus(JSONObject value) throws Failure {
        String deviceId = optionalString(value.opt("device_id"));
        if (deviceId != null && !CloudClipboardE2e.validDeviceId(deviceId)) throw new Failure("invalid_response");
        Boolean approved = JsonPolicy.strictBoolean(value.opt("approved"));
        Boolean joined = JsonPolicy.strictBoolean(value.opt("joined"));
        Boolean skew = JsonPolicy.strictBoolean(value.opt("clock_skew"));
        if (approved == null || joined == null || skew == null) throw new Failure("invalid_response");
        Object rawHeld = value.opt("held_deletions");
        Integer held = rawHeld == null || rawHeld == JSONObject.NULL ? null : JsonPolicy.nonNegativeInteger(rawHeld);
        Object rawLast = value.opt("last_success_ms");
        Long last = rawLast == null || rawLast == JSONObject.NULL ? null : JsonPolicy.nonNegativeLong(rawLast);
        return new Status(deviceId, optionalString(value.opt("code")), approved, joined, count(value.opt("words")),
            count(value.opt("pending_upload")), count(value.opt("pending_apply")), held, skew, last,
            optionalString(value.opt("last_error")));
    }

    /** 一个可以为 null 的字符串字段：缺失、null 或空串都当作没有；不是字符串时也当作没有，不让整页作废。 */
    private static String optionalString(Object raw) {
        if (raw == null || raw == JSONObject.NULL) return null;
        String value = JsonPolicy.strictString(raw);
        return value == null || value.isEmpty() ? null : value;
    }

    private static int count(Object raw) {
        return JsonPolicy.nonNegativeInteger(raw, 0);
    }
}
