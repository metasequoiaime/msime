package app.msime.android;

import android.content.Context;
import android.os.Build;
import app.msime.android.clipboard.CloudClipboardRelayPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.nio.file.Paths;
import java.security.GeneralSecurityException;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 经用户自建的中继（`docs/cloud-clipboard-relay.md`）读写云剪贴板，做法与水杉账号路径（{@link CloudClipboardApi} 的账号分支）相同：端到端加密的操作（列表、上传、设备）由 {@link CloudClipboardE2e#run} 跑共享驱动，密钥串由 {@link CloudClipboardKeystore} 用 AndroidKeyStore 加密保存，只是每个请求经共享层的 `msime_client_relay_cloud_clipboard_exchange` 发给中继（Rust 校验地址、限定路径、带上令牌、限制响应）。不涉及内容、不碰密钥串的动作（开关、置顶、保留、删除、清空）交给 `msime_client_relay_cloud_clipboard`。中继的地址和令牌在共享偏好的 `cloud_clipboard_relay` 里，{@link #open} 时读一次，这一页之后的请求都发往那一个中继。
 *
 * <p>偏好里配置了中继就只走中继（{@link #selected}，以共享层的 `msime_client_cloud_clipboard_source` 为准），中继出错也不退回水杉账号：否则用户以为在用自己的中继，内容却去了官方服务。设置页、键盘面板和账号页都先问这里再决定走哪条路。
 *
 * <p>中继的密钥串（`cloud-clipboard-relay-keystore`）放在 {@link #keystoreDirectory}，也就是 `noBackupFilesDir/cloud-clipboard-relay/`：主进程（设置页）和 `:ime` 进程（键盘）用同一个文件，同一台设备只登记一次。它和账号的那份一样用 AndroidKeyStore 里不导出的密钥加密（另一把密钥），而且在 noBackupFilesDir 下，云备份和设备间迁移都不带它；即使文件被拷走，没有这台设备的 AndroidKeyStore 也解不开。不经 `msime_client_relay_cloud_clipboard` 跑端到端加密的操作：那条路由 Rust 把密钥串写成只靠文件权限保护的明文文件。退出水杉账号不删除它；换了中继地址或令牌，旧的那份当作没有（见 {@link CloudClipboardKeystore#forRelay}）。
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
    /** 打开时偏好里的中继地址和令牌，这一页的每个请求都发往它。令牌是凭据，不写日志。 */
    private final String url;
    private final String token;
    private final CloudClipboardE2e.Local local;

    private CloudClipboardRelay(String preferencesDirectory, String keystoreDirectory, String deviceName,
            Relay relay, CloudClipboardKeystore.Sealer sealer) {
        this.preferencesDirectory = preferencesDirectory;
        this.keystoreDirectory = keystoreDirectory;
        this.deviceName = deviceName;
        this.binding = relay.binding();
        this.url = relay.url();
        this.token = relay.token();
        this.local = new CloudClipboardE2e.Local(CloudClipboardNativeDriver.INSTANCE,
            CloudClipboardKeystore.forRelay(Paths.get(keystoreDirectory), binding, sealer), deviceName);
    }

    /** 偏好里的一份中继配置和它的绑定。 */
    private record Relay(String url, String token, String binding) {}

    /**
     * 按偏好里现在的中继打开：记下这一刻的地址、令牌和绑定（{@link CloudClipboardRelayPolicy#binding}），之后据此认出地址或令牌换过。没有配置中继时抛出 `relay_not_configured`，偏好读不了时抛出 `storage`。读文件，要在工作线程上调用。
     */
    public static CloudClipboardRelay open(Context context) throws CloudApi.Failure {
        return open(preferencesDirectory(context), keystoreDirectory(context), deviceName(),
            CloudClipboardKeystore.relaySealer(context));
    }

    /** 同上，目录、设备名和加密密钥串的 AndroidKeyStore 密钥由调用方给出；设备冒烟用临时目录经它跑这一层。 */
    static CloudClipboardRelay open(String preferencesDirectory, String keystoreDirectory, String deviceName,
            CloudClipboardKeystore.Sealer sealer) throws CloudApi.Failure {
        Relay relay = currentRelay(preferencesDirectory);
        if (relay == null) throw failure(CloudClipboardRelayPolicy.CODE_NOT_CONFIGURED);
        return new CloudClipboardRelay(preferencesDirectory, keystoreDirectory, deviceName, relay, sealer);
    }

    /**
     * 键盘面板在后台线程上真正发请求之前的选路。`relay` 和 `binding` 是面板在主线程上按内存里的偏好快照记下的（{@link #binding(JSONObject)}），快照可能晚于偏好文件，所以这里再问一次共享层：两边都说走水杉账号时返回 null，调用方经账号发；两边都是中继且绑定相同时返回这条中继；其余情况（中继设置刚改过）一律抛出 `relay_changed`，绝不在配置了中继时退回账号，也不发到刚换掉的中继。读文件，要在工作线程上调用。
     */
    public static CloudClipboardRelay forPanel(Context context, boolean relay, String binding) throws CloudApi.Failure {
        return forPanel(preferencesDirectory(context), keystoreDirectory(context), deviceName(),
            CloudClipboardKeystore.relaySealer(context), relay, binding);
    }

    /** 同上，目录、设备名和 AndroidKeyStore 密钥由调用方给出；设备冒烟用临时目录经它核对选路。 */
    static CloudClipboardRelay forPanel(String preferencesDirectory, String keystoreDirectory, String deviceName,
            CloudClipboardKeystore.Sealer sealer, boolean relay, String binding) throws CloudApi.Failure {
        if (relay != selected(preferencesDirectory)) throw failure(CloudClipboardRelayPolicy.CODE_CHANGED);
        if (!relay) return null;
        CloudClipboardRelay route = open(preferencesDirectory, keystoreDirectory, deviceName, sealer);
        if (!route.binding.equals(binding)) throw failure(CloudClipboardRelayPolicy.CODE_CHANGED);
        return route;
    }

    /** 共享偏好目录，与键盘读的是同一个 runtime option `preferences_directory`；首次设置之前为空串。 */
    public static String preferencesDirectory(Context context) {
        File files = context.getFilesDir();
        return files == null ? "" : HostOptionsPolicy.readOption(files, "preferences_directory");
    }

    /** 本机中继密钥串的目录：`noBackupFilesDir/cloud-clipboard-relay`，第一次用到时由 {@link CloudClipboardKeystore} 建成只有本应用能进。 */
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
        Relay relay = currentRelay(preferencesDirectory);
        return relay == null ? "" : relay.binding();
    }

    /** 偏好文件里现在的中继；没配置时为 null，读不了时抛出 `storage`。 */
    private static Relay currentRelay(String preferencesDirectory) throws CloudApi.Failure {
        if (preferencesDirectory == null || preferencesDirectory.isEmpty()) return null;
        JSONObject snapshot = value(NativeClient.loadPreferences(preferencesDirectory));
        JSONObject preferences = snapshot.optJSONObject("preferences");
        if (preferences == null) throw failure("storage");
        String binding = binding(preferences);
        if (binding.isEmpty()) return null;
        JSONObject relay = preferences.optJSONObject(CloudClipboardRelayPolicy.PREFERENCE);
        return new Relay(JsonPolicy.strictStringOrEmpty(relay.opt("url")),
            JsonPolicy.strictStringOrEmpty(relay.opt("token")), binding);
    }

    /**
     * 跑一个端到端加密的操作，与账号路径同一个循环（{@link CloudClipboardE2e#run}）：驱动交来的密钥串在发下一个请求之前加密写进 {@link CloudClipboardKeystore}，写之前确认偏好里还是这个中继（否则 `relay_changed`，不写）。驱动报的错按 {@link CloudClipboardRelayPolicy#driverFailureCode} 归类，与 Rust 那条路给出的码相同。
     */
    @Override public String drive(CloudClipboardE2e.Operation operation) throws CloudApi.Failure {
        try {
            return CloudClipboardE2e.run(local, operation, this::exchange, this::requireCurrent);
        } catch (CloudClipboardE2e.Failure failure) {
            throw failure(CloudClipboardRelayPolicy.driverFailureCode(failure.status, failure.code));
        } catch (CloudApi.Failure failure) {
            throw failure;
        } catch (java.io.IOException | GeneralSecurityException storage) {
            // 密钥串文件或 AndroidKeyStore 读写失败；原因不带出去。
            throw failure(CloudClipboardRelayPolicy.CODE_STORAGE);
        } catch (Exception | LinkageError unexpected) {
            throw failure("invalid_response");
        }
    }

    /** 把驱动的一个请求发给打开时的那个中继，状态码和正文原样交回驱动。请求里有令牌，不写日志。 */
    private CloudClipboardE2e.Reply exchange(String method, String path, String body) throws CloudApi.Failure {
        JSONObject value;
        try {
            JSONObject request = new JSONObject()
                .put("url", url)
                .put("token", token)
                .put("method", method)
                .put("path", path);
            if (body != null) request.put("body", body);
            value = value(NativeClient.relayCloudClipboardExchange(request.toString()));
        } catch (JSONException | IllegalArgumentException malformed) {
            throw failure("invalid_cloud_clipboard");
        }
        Integer status = JsonPolicy.strictInteger(value.opt("status"));
        String text = JsonPolicy.strictString(value.opt("body"));
        if (status == null || status < 100 || status > 599 || text == null) throw failure("invalid_response");
        return new CloudClipboardE2e.Reply(status, text);
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
     * 跑一个不涉及内容、不碰密钥串的动作（{@link #direct}、{@link #setPinned}），返回结果。请求里没有凭据（令牌由 Rust 从偏好里读）。中继的错误以 {@link CloudApi.Failure} 抛出，`status` 为 0、`code` 是共享层给的码：`relay_unauthorized` 不是水杉账号未登录，调用方不能按「请登录」处理。端到端加密的动作不经这里（见类注释）。
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
