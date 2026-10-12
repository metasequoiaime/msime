package app.msime.android;

import android.content.Context;
import android.os.Build;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 经用户自建的中继（`docs/cloud-clipboard-relay.md`）读写云剪贴板：每个动作交给共享层的 `msime_client_relay_cloud_clipboard`，由 Rust 联网、在本机加密解密、保存中继的密钥串，这里只拼请求、读结果。中继的地址和令牌在共享偏好的 `cloud_clipboard_relay` 里，Rust 每次都重新读。
 *
 * <p>偏好里配置了中继就只走中继（{@link #selected}，以共享层的 `msime_client_cloud_clipboard_source` 为准），中继出错也不退回水杉账号：否则用户以为在用自己的中继，内容却去了官方服务。设置页、键盘面板和账号页都先问这里再决定走哪条路。
 *
 * <p>中继的密钥串（`cloud-clipboard-relay-keystore`）放在 {@link #keystoreDirectory}，也就是 `noBackupFilesDir/cloud-clipboard-relay/`：主进程（设置页）和 `:ime` 进程（键盘）用同一个目录，同一台设备只登记一次；这个目录不进自动备份和设备迁移。它是靠文件权限保护的明文文件，弱于账号路径用 AndroidKeyStore 加密的那份（{@link CloudClipboardKeystore}），这是各平台中继的共同取舍，见决策笔记。退出水杉账号不删除它。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它。每个动作都阻塞在网络上，不要在主线程或输入线程上调用。
 */
public final class CloudClipboardRelay implements CloudClipboardApi.Backend {
    /** 中继密钥串所在的目录名，在 noBackupFilesDir 下面。 */
    public static final String KEYSTORE_DIRECTORY = "cloud-clipboard-relay";

    private final String preferencesDirectory;
    private final String keystoreDirectory;
    private final String deviceName;
    private final String binding;

    private CloudClipboardRelay(String preferencesDirectory, String keystoreDirectory, String deviceName,
            String binding) {
        this.preferencesDirectory = preferencesDirectory;
        this.keystoreDirectory = keystoreDirectory;
        this.deviceName = deviceName;
        this.binding = binding;
    }

    /**
     * 按偏好里现在的中继打开：记下这一刻的绑定（{@link CloudClipboardRelayPolicy#binding}），之后据此认出地址或令牌换过。没有配置中继时抛出 `relay_not_configured`，偏好读不了时抛出 `storage`。读文件，要在工作线程上调用。
     */
    public static CloudClipboardRelay open(Context context) throws CloudApi.Failure {
        String directory = preferencesDirectory(context);
        String binding = currentBinding(directory);
        if (binding.isEmpty()) throw failure(CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED);
        return new CloudClipboardRelay(directory, keystoreDirectory(context), deviceName(), binding);
    }

    /** 共享偏好目录，与键盘读的是同一个 runtime option `preferences_directory`；首次设置之前为空串。 */
    public static String preferencesDirectory(Context context) {
        File files = context.getFilesDir();
        return files == null ? "" : HostOptionsPolicy.readOption(files, "preferences_directory");
    }

    /** 本机中继密钥串的目录：`noBackupFilesDir/cloud-clipboard-relay`，由 Rust 在第一次用到时建成仅本人可读写。 */
    public static String keystoreDirectory(Context context) {
        return new File(context.getNoBackupFilesDir(), KEYSTORE_DIRECTORY).getAbsolutePath();
    }

    /**
     * 云剪贴板这时该走中继还是水杉账号，以共享层为准。偏好目录还没有时（首次设置之前）不可能配置过中继，按水杉账号处理。读不了偏好（`storage`）或中继配置不合规（`relay_not_configured`）时抛出，调用方按失败处理，不改走账号。只读文件、不联网，但要在工作线程上调用。
     */
    public static boolean selected(String preferencesDirectory) throws CloudApi.Failure {
        if (preferencesDirectory == null || preferencesDirectory.isEmpty()) return false;
        JSONObject value = value(NativeClient.cloudClipboardSource(preferencesDirectory));
        CloudClipboardRelayPolicy.Route route = CloudClipboardRelayPolicy.route(
            JsonPolicy.strictString(value.opt("source")));
        if (route == null) throw failure("invalid_response");
        return route == CloudClipboardRelayPolicy.Route.RELAY;
    }

    /** 偏好文档（`preferences` 对象）里中继的绑定；没配置或文档为 null 时为空串。不读文件，主线程上可以用。 */
    public static String binding(JSONObject preferences) {
        JSONObject relay = preferences == null ? null : preferences.optJSONObject(CloudClipboardRelayPolicy.PREFERENCE);
        if (relay == null) return "";
        return CloudClipboardRelayPolicy.binding(JsonPolicy.strictStringOrEmpty(relay.opt("url")),
            JsonPolicy.strictStringOrEmpty(relay.opt("token")));
    }

    /** 偏好文件里现在的中继绑定；没配置时为空串，读不了时抛出 `storage`。读文件，要在工作线程上调用。 */
    public static String currentBinding(String preferencesDirectory) throws CloudApi.Failure {
        if (preferencesDirectory == null || preferencesDirectory.isEmpty()) return "";
        JSONObject snapshot = value(NativeClient.loadPreferences(preferencesDirectory));
        JSONObject preferences = snapshot.optJSONObject("preferences");
        if (preferences == null) throw failure("storage");
        return binding(preferences);
    }

    /** 打开时记下的绑定，键盘面板拿它代替账号编号。 */
    public String binding() {
        return binding;
    }

    /** 驱动的操作换成中继的动作：`status` 在这里叫 `devices`，结果形状与驱动的相同。 */
    @Override public String drive(CloudClipboardE2e.Operation operation) throws CloudApi.Failure {
        try {
            JSONObject action = new JSONObject();
            switch (operation.op()) {
                case "list" -> action.put("operation", "list").put("search", operation.search());
                case "add" -> action.put("operation", "add").put("text", operation.text());
                case "status" -> action.put("operation", "devices");
                case "approve", "reject", "remove" ->
                    action.put("operation", operation.op()).put("device_id", operation.deviceId());
                case "reset" -> action.put("operation", "reset");
                default -> throw new IllegalArgumentException("unsupported cloud clipboard operation");
            }
            return call(action).toString();
        } catch (JSONException impossible) {
            throw failure("invalid_cloud_clipboard");
        }
    }

    @Override public void setEnabled(boolean enabled) throws CloudApi.Failure {
        direct("set_enabled", "enabled", enabled);
    }

    @Override public void setRetention(int days) throws CloudApi.Failure {
        direct("set_retention", "days", days);
    }

    @Override public void setPinned(String id, boolean pinned) throws CloudApi.Failure {
        try {
            call(new JSONObject().put("operation", "pin").put("id", id).put("pinned", pinned));
        } catch (JSONException impossible) {
            throw failure("invalid_cloud_clipboard");
        }
    }

    @Override public void delete(String id) throws CloudApi.Failure {
        direct("delete", "id", id);
    }

    @Override public void clear() throws CloudApi.Failure {
        direct("clear", null, null);
    }

    /** 偏好里的中继还是打开时那一个；换了地址或令牌、改回了水杉账号都抛出 `relay_changed`。 */
    @Override public void requireCurrent() throws CloudApi.Failure {
        if (!binding.equals(currentBinding(preferencesDirectory))) throw failure(CloudClipboardRelayPolicy.CODE_CHANGED);
    }

    private void direct(String operation, String key, Object value) throws CloudApi.Failure {
        try {
            JSONObject action = new JSONObject().put("operation", operation);
            if (key != null) action.put(key, value);
            call(action);
        } catch (JSONException impossible) {
            throw failure("invalid_cloud_clipboard");
        }
    }

    /**
     * 跑一个动作，返回结果。请求里没有凭据（令牌由 Rust 从偏好里读），但结果里有解密后的剪贴板内容，不写日志。中继的错误以 {@link CloudApi.Failure} 抛出，`status` 为 0、`code` 是共享层给的码：`relay_unauthorized` 不是水杉账号未登录，调用方不能按「请登录」处理。
     */
    private JSONObject call(JSONObject action) throws CloudApi.Failure, JSONException {
        JSONObject request = new JSONObject()
            .put("preferences_directory", preferencesDirectory)
            .put("directory", keystoreDirectory)
            .put("device_name", deviceName)
            .put("action", action);
        return value(NativeClient.relayCloudClipboard(request.toString()));
    }

    /** 拆共享层的 `{"ok":true,"value":{…}}`；`ok` 不是真正的 true 时以 `error` 为错误码抛出。 */
    private static JSONObject value(String response) throws CloudApi.Failure {
        try {
            JSONObject root = new JSONObject(response);
            if (!JsonPolicy.strictTrue(root.opt("ok"))) {
                String code = JsonPolicy.strictString(root.opt("error"));
                throw failure(code == null || code.isEmpty() ? "invalid_response" : code);
            }
            JSONObject value = root.optJSONObject("value");
            if (value == null) throw failure("invalid_response");
            return value;
        } catch (JSONException | RuntimeException | LinkageError malformed) {
            // 共享库没加载上或回答读不懂：原因不带出去，里面可能有剪贴板内容。
            throw failure("invalid_response");
        }
    }

    private static CloudApi.Failure failure(String code) {
        return new CloudApi.Failure(0, code, code, 0);
    }

    private static String deviceName() {
        return CloudClipboardE2e.deviceName(Build.MANUFACTURER, Build.MODEL);
    }
}
