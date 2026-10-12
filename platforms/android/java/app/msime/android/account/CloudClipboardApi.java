package app.msime.android;

import app.msime.android.clipboard.CloudClipboardTextPolicy;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「我的 → 云剪贴板」页的接口：列表、设备与批准、开关、保留时长、置顶、删除与清空。
 *
 * <p>条目端到端加密（`docs/cloud-clipboard-e2e.md`）：文字只在设备上加密和解密，服务端只保存密文，搜索也在解密之后于本机进行。读列表、读设备和「允许」「拒绝」「移除」「重置」都经共享驱动（{@link CloudClipboardE2e}）完成；置顶、保留时长、删除、清空和开关不涉及内容，仍直接调用原来的接口。只有真实账号有云剪贴板。服务端最多保存 50 条，单条不超过 4000 个 UTF-16 单元；保留时长 0 表示一直保留。
 *
 * <p>请求经哪条路发出由 {@link Backend} 决定：水杉账号（{@link #CloudClipboardApi(CloudApi, CloudClipboardE2e.Local)}）或用户自建的中继（{@link CloudClipboardRelay}，`docs/cloud-clipboard-relay.md`）。两条路的结果形状相同，解析和校验只在这里写一份。
 *
 * <p>本类不 import `androidx`、`R` 或 `home/`，check-host 会编译它；请求阻塞在网络上，不要在主线程调用。
 */
public final class CloudClipboardApi {
    public static final String PATH = "/v1/users/me/clipboard";
    public static final int MAX_ITEMS = 50;
    /** 服务端每个账号最多登记的设备数。 */
    public static final int MAX_DEVICES = 20;
    /** 保留时长的可选值（天），0 表示一直保留。 */
    public static final List<Integer> RETENTION_DAYS = List.of(1, 7, 30, 0);
    /** 本机可以读写。 */
    public static final String STATUS_READY = "ready";
    /** 本机还在等已有设备批准，读不到也传不了。 */
    public static final String STATUS_PENDING = "pending";
    /** 账号没有开云剪贴板。 */
    public static final String STATUS_DISABLED = "disabled";

    /** 一条记录。`device` 是写入时的设备名，未知时为空字符串。 */
    public record Item(String id, String text, String updatedAt, boolean pinned, String device) {}

    /** 本机的加密状态：`status` 见上面三个常量，`code` 是本机的核对码，`pendingDevices` 是等本机批准的设备数，`undecryptable` 是本机解不开而没有列出的条数。 */
    public record E2e(String status, String code, int pendingDevices, int undecryptable) {}

    /** 一次读取的结果。`retentionDays` 读不到时（本机待批准）为 -1。 */
    public record Page(boolean enabled, int retentionDays, List<Item> items, E2e e2e) {}

    /** 账号下登记的一台设备。`current` 标出本机，`code` 是它的核对码，`createdAt` 读不到时为空字符串。 */
    public record Device(String deviceId, String name, String createdAt, boolean approved, boolean current, String code) {}

    /** 设备列表与本机状态。 */
    public record Devices(boolean enabled, String status, String deviceId, String code, List<Device> devices) {
        /** 已登记、还没被批准的其他设备。 */
        public List<Device> pending() {
            List<Device> pending = new ArrayList<>(devices.size());
            for (Device device : devices) if (!device.current() && !device.approved()) pending.add(device);
            return pending;
        }

        /** 已批准的设备，本机在内。 */
        public List<Device> approved() {
            List<Device> approved = new ArrayList<>(devices.size());
            for (Device device : devices) if (device.approved()) approved.add(device);
            return approved;
        }
    }

    /** 设置页一次读到的全部：设备与本机状态，以及（本机能读时）列表。 */
    public record Overview(Devices devices, Page page) {}

    /**
     * 云剪贴板的一条路：水杉账号（{@link AccountBackend}）或自建中继（{@link CloudClipboardRelay}）。每个方法都阻塞在网络上；失败一律以 {@link CloudApi.Failure} 抛出，`code` 是驱动、账号服务或中继给的错误码。
     */
    public interface Backend {
        /** 跑一个端到端加密的操作，返回驱动结果的 JSON 文本，形状见 `docs/cloud-clipboard-e2e.md`。 */
        String drive(CloudClipboardE2e.Operation operation) throws CloudApi.Failure;

        void setEnabled(boolean enabled) throws CloudApi.Failure;

        /** `days` 已由调用方核对过（{@link #validRetention}）。 */
        void setRetention(int days) throws CloudApi.Failure;

        /** `id` 已由调用方核对过（{@link #validId}）。 */
        void setPinned(String id, boolean pinned) throws CloudApi.Failure;

        /** `id` 已由调用方核对过（{@link #validId}）。 */
        void delete(String id) throws CloudApi.Failure;

        void clear() throws CloudApi.Failure;

        /** 确认这一页打开时的那条路还在：账号是同一个登录，中继是同一个地址和令牌。 */
        void requireCurrent() throws CloudApi.Failure;
    }

    private final Backend backend;

    /** 经水杉账号：请求走 `api` 绑定的那个登录（{@link CloudApi#forAccountSession}）。 */
    public CloudClipboardApi(CloudApi api, CloudClipboardE2e.Local local) {
        this(new AccountBackend(api, local));
    }

    public CloudClipboardApi(Backend backend) {
        this.backend = backend;
    }

    /** Recheck the page's login (or, on a relay, its address and token) before copying one of its rows to the system clipboard. */
    public void requireCurrentSession() throws CloudApi.Failure {
        backend.requireCurrent();
    }

    public static boolean validId(String id) {
        return TextPolicy.isLowerHex(id, 64);
    }

    public static boolean validRetention(int days) {
        return RETENTION_DAYS.contains(days);
    }

    /** 置顶的排在前面，其余保持服务端给的顺序（新的在前）。 */
    public static List<Item> ordered(List<Item> items) {
        int capacity = items == null ? 0 : items.size();
        List<Item> ordered = new ArrayList<>(capacity);
        for (Item item : items) if (item.pinned()) ordered.add(item);
        for (Item item : items) if (!item.pinned()) ordered.add(item);
        return ordered;
    }

    /** 先读设备与本机状态（必要时登记本机、建立密钥），本机能读时再读列表。 */
    public Overview load() throws CloudApi.Failure {
        Devices devices = devices();
        if (!devices.enabled()) {
            return new Overview(devices, new Page(false, 0, List.of(), new E2e(STATUS_DISABLED, devices.code(), 0, 0)));
        }
        if (!STATUS_READY.equals(devices.status())) {
            return new Overview(devices, new Page(true, -1, List.of(),
                new E2e(devices.status(), devices.code(), devices.pending().size(), 0)));
        }
        return new Overview(devices, list(""));
    }

    /** 解密后的列表；`search` 在本机过滤，服务端不再搜索。 */
    public Page list(String search) throws CloudApi.Failure {
        return parsePage(backend.drive(CloudClipboardE2e.Operation.list(search)));
    }

    /** 设备列表与本机状态（驱动的 `status`）。 */
    public Devices devices() throws CloudApi.Failure {
        return parseDevices(backend.drive(CloudClipboardE2e.Operation.status()));
    }

    /** 把剪贴板密钥封装给这台设备。调用前要让用户核对两台设备上的核对码。 */
    public Devices approve(String deviceId) throws CloudApi.Failure {
        return parseDevices(backend.drive(CloudClipboardE2e.Operation.approve(deviceId)));
    }

    /** 删掉一台待批准设备的登记。 */
    public Devices reject(String deviceId) throws CloudApi.Failure {
        return parseDevices(backend.drive(CloudClipboardE2e.Operation.reject(deviceId)));
    }

    /** 移除一台已批准的设备：换一把新密钥、删掉云端全部记录，其他设备都要重新批准。 */
    public Devices remove(String deviceId) throws CloudApi.Failure {
        return parseDevices(backend.drive(CloudClipboardE2e.Operation.remove(deviceId)));
    }

    /** 重置：换一把只封装给本机的新密钥、删掉云端全部记录，其他设备都要重新批准。 */
    public Devices reset() throws CloudApi.Failure {
        return parseDevices(backend.drive(CloudClipboardE2e.Operation.reset()));
    }

    public void setEnabled(boolean enabled) throws CloudApi.Failure {
        backend.setEnabled(enabled);
    }

    public void setRetention(int days) throws CloudApi.Failure {
        if (!validRetention(days)) throw new IllegalArgumentException("unsupported retention " + days);
        backend.setRetention(days);
    }

    public void setPinned(String id, boolean pinned) throws CloudApi.Failure {
        if (!validId(id)) throw new IllegalArgumentException("invalid clipboard id");
        backend.setPinned(id, pinned);
    }

    public void delete(String id) throws CloudApi.Failure {
        if (!validId(id)) throw new IllegalArgumentException("invalid clipboard id");
        backend.delete(id);
    }

    /** 清空全部记录（开关保持不变）。 */
    public void clear() throws CloudApi.Failure {
        backend.clear();
    }

    /** 经水杉账号的那条路：驱动的请求和不涉及内容的几个接口都走同一个绑定好的登录。 */
    private static final class AccountBackend implements Backend {
        private final CloudApi api;
        private final CloudClipboardE2e.Local local;

        AccountBackend(CloudApi api, CloudClipboardE2e.Local local) {
            this.api = api;
            this.local = local;
        }

        /**
         * 经驱动跑一个操作。请求走这一页绑定的登录（{@link CloudApi#forAccountSession}），密钥串只在这个登录还在时写入；驱动的错误码原样放进 {@link CloudApi.Failure#code}。
         */
        @Override public String drive(CloudClipboardE2e.Operation operation) throws CloudApi.Failure {
            try {
                return CloudClipboardE2e.run(local, operation, this::exchange, api::currentAccountSessionId);
            } catch (CloudClipboardE2e.Failure failure) {
                throw new CloudApi.Failure(failure.status, failure.code, failure.code, 0);
            } catch (CloudApi.Failure failure) {
                throw failure;
            } catch (java.util.concurrent.CancellationException changed) {
                throw new CloudApi.Failure(409, "session_changed", "account session changed", 0);
            } catch (java.io.IOException storage) {
                throw new CloudApi.Failure(0, "secure_storage", "cloud clipboard keystore unavailable", 0);
            } catch (Exception | LinkageError unexpected) {
                throw invalid();
            }
        }

        @Override public void setEnabled(boolean enabled) throws CloudApi.Failure {
            api.json("PUT", PATH + "/settings", object("enabled", enabled), CloudApi.Auth.ACCOUNT);
        }

        @Override public void setRetention(int days) throws CloudApi.Failure {
            api.json("PUT", PATH + "/retention", object("days", days), CloudApi.Auth.ACCOUNT);
        }

        @Override public void setPinned(String id, boolean pinned) throws CloudApi.Failure {
            api.json("PUT", PATH + "/" + id + "/pin", object("pinned", pinned), CloudApi.Auth.ACCOUNT);
        }

        @Override public void delete(String id) throws CloudApi.Failure {
            api.send("DELETE", PATH + "/" + id, null, CloudApi.Auth.ACCOUNT);
        }

        @Override public void clear() throws CloudApi.Failure {
            api.send("DELETE", PATH, null, CloudApi.Auth.ACCOUNT);
        }

        @Override public void requireCurrent() throws CloudApi.Failure {
            api.currentAccountSessionId();
        }

        private CloudClipboardE2e.Reply exchange(String method, String path, String body) throws CloudApi.Failure {
            CloudApi.Exchange exchange = api.exchange(method, path,
                body == null ? null : new CloudApi.Body("application/json", TextPolicy.utf8Bytes(body)),
                CloudApi.Auth.ACCOUNT);
            byte[] raw = exchange.body();
            if (raw != null && raw.length > CloudClipboardE2e.MAX_RESPONSE_BYTES) throw invalid();
            return new CloudClipboardE2e.Reply(exchange.status(), TextPolicy.utf8(raw));
        }
    }

    /** 驱动 `list` 的结果：`{"enabled","retention_days","items":[…],"e2e":{…}}`。 */
    static Page parsePage(String json) throws CloudApi.Failure {
        try {
            JSONObject response = new JSONObject(json);
            JSONArray values = response.optJSONArray("items");
            Object enabled = response.opt("enabled");
            if (values == null || values.length() > MAX_ITEMS || !(enabled instanceof Boolean)) throw invalid();
            Integer rawRetention = JsonPolicy.strictInteger(response.opt("retention_days"));
            int retention = rawRetention == null ? 0 : rawRetention;
            if (!validRetention(retention)) retention = 0;
            List<Item> items = new ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) items.add(item(values.getJSONObject(index)));
            JSONObject e2e = response.optJSONObject("e2e");
            if (e2e == null) throw invalid();
            String status = status(e2e.opt("status"));
            String code = code(e2e.opt("code"));
            Integer pending = JsonPolicy.nonNegativeInteger(e2e.opt("pending_devices"));
            Integer undecryptable = JsonPolicy.nonNegativeInteger(e2e.opt("undecryptable"));
            if (pending == null || undecryptable == null) throw invalid();
            return new Page((Boolean) enabled, retention, ordered(items),
                new E2e(status, code, Math.min(pending, MAX_DEVICES), Math.min(undecryptable, MAX_ITEMS)));
        } catch (JSONException malformed) {
            throw invalid();
        }
    }

    /** 驱动 `status` 一类操作的结果：`{"enabled","status","device_id","code","devices":[…]}`。 */
    static Devices parseDevices(String json) throws CloudApi.Failure {
        try {
            JSONObject response = new JSONObject(json);
            Object enabled = response.opt("enabled");
            JSONArray values = response.optJSONArray("devices");
            String own = JsonPolicy.strictString(response.opt("device_id"));
            if (!(enabled instanceof Boolean) || values == null || values.length() > MAX_DEVICES
                    || !CloudClipboardE2e.validDeviceId(own)) throw invalid();
            List<Device> devices = new ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) {
                JSONObject value = values.getJSONObject(index);
                String id = JsonPolicy.strictString(value.opt("device_id"));
                Boolean approved = JsonPolicy.strictBoolean(value.opt("approved"));
                Boolean current = JsonPolicy.strictBoolean(value.opt("current"));
                if (!CloudClipboardE2e.validDeviceId(id) || approved == null || current == null) throw invalid();
                devices.add(new Device(id, label(value.opt("name")), label(value.opt("created_at")),
                    approved, current, code(value.opt("code"))));
            }
            return new Devices((Boolean) enabled, status(response.opt("status")), own,
                code(response.opt("code")), List.copyOf(devices));
        } catch (JSONException malformed) {
            throw invalid();
        }
    }

    private static Item item(JSONObject value) throws CloudApi.Failure {
        String id = JsonPolicy.strictString(value.opt("id"));
        String text = JsonPolicy.strictString(value.opt("text"));
        String updated = JsonPolicy.strictString(value.opt("updated_at"));
        Object rawPinned = value.opt("pinned");
        Boolean pinned = rawPinned == null || rawPinned == JSONObject.NULL
            ? Boolean.FALSE : JsonPolicy.strictBoolean(rawPinned);
        if (pinned == null || id == null || text == null || updated == null) throw invalid();
        if (!validId(id) || !CloudClipboardTextPolicy.valid(text)
                || !TextPolicy.boundedNonEmpty(updated, 128)) throw invalid();
        return new Item(id, text, updated, pinned, label(value.opt("device")));
    }

    /** 一条元数据文字（设备名、登记时间）：不是字符串、过长、带控制字符时按空字符串处理，不让整页作废。 */
    private static String label(Object raw) {
        String value = raw == null || raw == JSONObject.NULL ? "" : JsonPolicy.strictString(raw);
        if (value == null || TextPolicy.utf8Length(value) > 256 || TextPolicy.hasControl(value)
                || !TextPolicy.validUnicode(value)) return "";
        return value;
    }

    private static String status(Object raw) throws CloudApi.Failure {
        String status = JsonPolicy.strictString(raw);
        if (STATUS_READY.equals(status) || STATUS_PENDING.equals(status) || STATUS_DISABLED.equals(status)) return status;
        throw invalid();
    }

    /** 核对码：16 位数字，每 4 位一组，组间一个空格。 */
    static boolean validCode(String code) {
        if (code == null || code.length() != 19) return false;
        for (int index = 0; index < code.length(); index++) {
            char c = code.charAt(index);
            if (index % 5 == 4 ? c != ' ' : !TextPolicy.isAsciiDigit(c)) return false;
        }
        return true;
    }

    private static String code(Object raw) throws CloudApi.Failure {
        String code = JsonPolicy.strictString(raw);
        if (!validCode(code)) throw invalid();
        return code;
    }

    private static JSONObject object(String key, Object value) {
        try {
            return new JSONObject().put(key, value);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
    }

    private static CloudApi.Failure invalid() {
        return new CloudApi.Failure(500, "invalid_response", "invalid clipboard response", 0);
    }

}
