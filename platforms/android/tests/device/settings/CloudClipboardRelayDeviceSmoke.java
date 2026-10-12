package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import app.msime.android.CloudClipboardApi;
import app.msime.android.CloudClipboardE2e;
import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.Callable;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 在应用进程里经 JNI 跑云剪贴板的自建中继（`msime_client_cloud_clipboard_source` 与 `msime_client_relay_cloud_clipboard`）：对面是本进程里一台只说 HTTP/1.1 的回环中继，背后是驱动冒烟里那个按协议写的假服务端（{@link CloudClipboardDriverDeviceSmoke.Server}）。
 *
 * <p>JVM 冒烟碰不到 Rust 的联网和落盘；这里验证 JNI 导出、请求形状、路径前缀、静态令牌、401 报成 `relay_unauthorized`（不是账号未登录），以及 Rust 在应用的 noBackupFilesDir 下建得起密钥串目录（SELinux 和目录权限只有设备上才有）。用的是临时的偏好目录和临时的密钥串目录，不碰应用自己的偏好、账号会话和密钥串，也不连外网。
 *
 * <p>前半段直接把手写的 JSON 交给 NativeClient，核对 Rust 一侧；后半段（{@link #glue}）经设置页和键盘实际用的 Java 这一层再走一遍：`CloudClipboardRelay` 的动作换名（`status` 叫 `devices`）、拆 `{ok,value}`、绑定和键盘的选路（{@code forPanel}），`CloudClipboardApi` 用中继的结果读页面和设备，`BackendAccount.clipboardPage` / `clipboardItem` 读中继的列表和上传结果。这一层改坏了，前半段照样通过，只有后半段会失败。
 */
public final class CloudClipboardRelayDeviceSmoke extends Instrumentation {
    private static final String TOKEN = "synthetic-relay-token-0001";
    private static final String PREFIX = "/msime";
    private static final String API = "/v1/users/me/clipboard";
    private static final String TEXT = "合成的中继剪贴板文本 synthetic";

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        File scratch = new File(getTargetContext().getCacheDir(), "cloud-clipboard-relay-smoke");
        File keysA = new File(getTargetContext().getNoBackupFilesDir(), "cloud-clipboard-relay-smoke-a");
        File keysB = new File(getTargetContext().getNoBackupFilesDir(), "cloud-clipboard-relay-smoke-b");
        File keysC = new File(getTargetContext().getNoBackupFilesDir(), "cloud-clipboard-relay-smoke-c");
        try (Relay relay = new Relay()) {
            run(relay, scratch, keysA, keysB, keysC);
            result.putString("stream",
                "MSIME_DEVICE_SMOKE_PASSED: cloud clipboard relay over JNI: source, prefix, bearer token, register, approve, encrypted add, list, pin, delete, unauthorized and unavailable; through CloudClipboardRelay, CloudClipboardApi and BackendAccount: keyboard route, settings overview, approve, remove, pin, retention, delete, clear and relay_changed\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError | LinkageError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: cloud clipboard relay ("
                + error.getClass().getSimpleName() + ": " + error.getMessage() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            delete(scratch);
            delete(keysA);
            delete(keysB);
            delete(keysC);
        }
    }

    private static void run(Relay relay, File scratch, File keysA, File keysB, File keysC) throws Exception {
        delete(scratch);
        File preferences = new File(scratch, "state");
        check(preferences.mkdirs(), "the scratch preferences directory is created");
        String directory = preferences.getAbsolutePath();

        // 没配置中继：走水杉账号，中继入口直接答 relay_not_configured，不联网。
        check("account".equals(value(source(directory)).getString("source")), "no relay means the account");
        check("relay_not_configured".equals(error(relay(directory, keysA, "Device A", list()))),
            "the relay entry refuses without a relay");
        // 键盘的选路：没配置中继时按账号读写（返回 null），面板以为是中继时不发；设置页打不开中继。
        check(forPanel(directory, keysA, false, "") == null, "without a relay the keyboard goes through the account");
        check(CODE_CHANGED.equals(failureCode(() -> forPanel(directory, keysA, true, STALE_BINDING))),
            "a keyboard that still thinks it is on a relay sends nothing once the relay is gone");
        check("relay_not_configured".equals(failureCode(() -> openRelay(directory, keysA, "Device A"))),
            "the settings page cannot open a relay that is not configured");

        String url = "http://127.0.0.1:" + relay.port() + PREFIX + "/";
        configure(directory, url, TOKEN);
        check("relay".equals(value(source(directory)).getString("source")), "a configured relay is the only route");
        check(relay.requests() == 0, "asking for the route sends nothing");

        // 第一台设备建钥；中继只见到带前缀的路径和静态令牌，Rust 在 noBackupFilesDir 下建出密钥串目录。
        JSONObject devices = value(relay(directory, keysA, "Device A", new JSONObject().put("operation", "devices")));
        check("ready".equals(devices.getString("status")), "the first device creates the key on the relay");
        check(new File(keysA, "cloud-clipboard-relay-keystore").isFile(), "the relay keystore is written under noBackupFilesDir");
        check(relay.requests() > 0 && relay.badRequests() == 0, "every request carried the path prefix and the bearer token");

        JSONObject added = value(relay(directory, keysA, "Device A", new JSONObject().put("operation", "add").put("text", TEXT)));
        check(TEXT.equals(added.getString("text")), "the uploaded item comes back decrypted");
        String ciphertext = relay.server.items.get(0).getString("ciphertext");
        check(ciphertext.startsWith("v1.") && !ciphertext.contains("synthetic"), "the relay holds only ciphertext");

        // 第二台设备待批准，核对码一致，批准后读得到第一台的条目。
        JSONObject pendingList = value(relay(directory, keysB, "Device B", list()));
        check("pending".equals(pendingList.getJSONObject("e2e").getString("status")), "a second device waits for approval");
        JSONObject seen = value(relay(directory, keysA, "Device A", new JSONObject().put("operation", "devices")));
        JSONObject waiting = null;
        JSONArray all = seen.getJSONArray("devices");
        for (int index = 0; index < all.length(); index++) {
            if (!all.getJSONObject(index).getBoolean("current")) waiting = all.getJSONObject(index);
        }
        check(waiting != null && "Device B".equals(waiting.getString("name")), "the approver sees the waiting device by name");
        check(pendingList.getJSONObject("e2e").getString("code").equals(waiting.getString("code")),
            "both devices show the same verification code");
        value(relay(directory, keysA, "Device A",
            new JSONObject().put("operation", "approve").put("device_id", waiting.getString("device_id"))));
        JSONObject readB = value(relay(directory, keysB, "Device B", list()));
        check(readB.getJSONArray("items").length() == 1
            && TEXT.equals(readB.getJSONArray("items").getJSONObject(0).getString("text")),
            "the approved device decrypts the first device's item");

        // 置顶和删除只在中继的动作集里，直接请求，不经驱动。
        String id = added.getString("id");
        value(relay(directory, keysA, "Device A", new JSONObject().put("operation", "pin").put("id", id).put("pinned", true)));
        check(relay.server.items.get(0).getBoolean("pinned"), "pin reaches the relay");
        value(relay(directory, keysA, "Device A", new JSONObject().put("operation", "delete").put("id", id)));
        check(relay.server.items.isEmpty(), "delete reaches the relay");

        String binding = relayBinding(directory);
        CloudClipboardApi settingsA = glue(relay, directory, keysA, keysC, binding);

        // 令牌被拒是 relay_unauthorized，不会被当成水杉账号未登录。
        configure(directory, url, "synthetic-relay-token-0002");
        check("relay_unauthorized".equals(error(relay(directory, keysA, "Device A", list()))),
            "a rejected token is relay_unauthorized");
        // 换了令牌：已打开的设置页和键盘按旧绑定发的请求都不再发出。
        check(CODE_CHANGED.equals(failureCode(() -> {
            settingsA.requireCurrentSession();
            return null;
        })), "a settings page opened on the old token is relay_changed");
        check(CODE_CHANGED.equals(failureCode(() -> forPanel(directory, keysA, true, binding))),
            "a keyboard holding the old binding sends nothing to the new relay");
        check(!binding.equals(relayBinding(directory)), "a new token is a new binding");
        // 没有服务的回环端口：连不上是 relay_unavailable。
        configure(directory, "http://127.0.0.1:" + relay.closedPort() + PREFIX, TOKEN);
        check("relay_unavailable".equals(error(relay(directory, keysA, "Device A", list()))),
            "an unreachable relay is relay_unavailable");
        check("invalid_cloud_clipboard".equals(error(relay(directory, keysA, "Device A",
            new JSONObject().put("operation", "pin").put("id", "short").put("pinned", true)))),
            "a malformed action is refused before the network");
    }

    /**
     * 经设置页和键盘实际用的 Java 这一层走一遍：此时中继上已有密钥，设备 A、B 已批准，没有条目。设备 C 是新的一台，经设置页的读法登记、等待、被 A 批准，最后被 A 移除。返回设备 A 的设置页接口，调用方换令牌后再用它核对 `relay_changed`。
     */
    private static CloudClipboardApi glue(Relay relay, String directory, File keysA, File keysC, String binding)
            throws Exception {
        check(binding.startsWith("relay:") && !binding.contains(TOKEN), "the binding names the relay without the token");
        int before = relay.requests();
        // 配置了中继：面板按账号来问被拒，绑定对不上被拒，都不联网。
        check(CODE_CHANGED.equals(failureCode(() -> forPanel(directory, keysA, false, "synthetic-account"))),
            "a configured relay never lets the keyboard fall back to the account");
        check(CODE_CHANGED.equals(failureCode(() -> forPanel(directory, keysA, true, STALE_BINDING))),
            "a binding from another relay is refused");
        check(relay.requests() == before, "refused routes send nothing");

        // 键盘：上传一条、读列表，都经 BackendAccount 解析中继的回答。
        CloudClipboardApi.Backend keyboard = forPanel(directory, keysA, true, binding);
        check(keyboard != null, "the keyboard on a relay gets the relay");
        BackendAccount.ClipboardItem uploaded = clipboardItem(keyboard.drive(CloudClipboardE2e.Operation.add(TEXT)));
        check(TEXT.equals(uploaded.text()) && CloudClipboardApi.validId(uploaded.id()), "the keyboard reads the relay's add result");
        BackendAccount.ClipboardPage panel = clipboardPage(keyboard.drive(CloudClipboardE2e.Operation.list("")));
        check(panel.enabled() && CloudClipboardApi.STATUS_READY.equals(panel.status()) && !panel.pending()
            && panel.items().size() == 1 && TEXT.equals(panel.items().get(0).text()), "the keyboard reads the relay's list");

        // 设置页：status 换名成 devices，再读列表。
        CloudClipboardApi settingsA = new CloudClipboardApi(openRelay(directory, keysA, "Device A"));
        settingsA.requireCurrentSession();
        CloudClipboardApi.Overview overviewA = settingsA.load();
        check(CloudClipboardApi.STATUS_READY.equals(overviewA.devices().status()) && overviewA.devices().approved().size() == 2
            && overviewA.devices().pending().isEmpty(), "the settings page reads the relay's devices");
        check(overviewA.page().enabled() && overviewA.page().items().size() == 1
            && uploaded.id().equals(overviewA.page().items().get(0).id()), "the settings page reads the relay's list");

        // 第三台设备经设置页登记并等待，A 在设置页上核对码、批准。
        CloudClipboardApi settingsC = new CloudClipboardApi(openRelay(directory, keysC, "Device C"));
        CloudClipboardApi.Overview waiting = settingsC.load();
        check(CloudClipboardApi.STATUS_PENDING.equals(waiting.devices().status()) && waiting.page().items().isEmpty()
            && waiting.page().retentionDays() == -1, "a new device waits on the settings page");
        List<CloudClipboardApi.Device> pending = settingsA.devices().pending();
        check(pending.size() == 1 && "Device C".equals(pending.get(0).name())
            && pending.get(0).code().equals(waiting.devices().code()), "the approver sees the new device and the same code");
        String idC = pending.get(0).deviceId();
        CloudClipboardApi.Devices approved = settingsA.approve(idC);
        check(approved.pending().isEmpty() && approved.approved().size() == 3, "approve goes through device_id");
        CloudClipboardApi.Page readC = settingsC.list("");
        check(readC.items().size() == 1 && TEXT.equals(readC.items().get(0).text()), "the approved device decrypts the list");
        check(settingsC.list("absent").items().isEmpty(), "search runs on the device");

        // 置顶、保留时长、删除、清空：中继专有的动作。
        settingsA.setPinned(uploaded.id(), true);
        check(relay.server.items.get(0).getBoolean("pinned") && settingsA.list("").items().get(0).pinned(),
            "pin goes through the relay and the list shows it");
        settingsA.setRetention(7);
        check(relay.retention() == 7, "retention goes through the relay");
        String second = clipboardItem(keyboard.drive(CloudClipboardE2e.Operation.add(TEXT + " 2"))).id();
        settingsA.delete(uploaded.id());
        check(relay.server.items.size() == 1 && second.equals(relay.server.items.get(0).getString("id")),
            "delete removes only that item");
        settingsA.clear();
        check(relay.server.items.isEmpty() && settingsA.list("").items().isEmpty(), "clear empties the relay");

        // 移除 C：换钥，C 不再在设备列表里。
        CloudClipboardApi.Devices removed = settingsA.remove(idC);
        boolean stillListed = false;
        for (CloudClipboardApi.Device device : removed.devices()) stillListed |= idC.equals(device.deviceId());
        check(!stillListed, "remove goes through device_id");
        return settingsA;
    }

    private static JSONObject list() throws Exception {
        return new JSONObject().put("operation", "list").put("search", "");
    }

    /** 往临时偏好目录写一份中继配置，走的是与设置页相同的共享保存（Rust 校验地址和令牌）。 */
    private static void configure(String directory, String url, String token) throws Exception {
        JSONObject snapshot = value(call("loadPreferences", new Class<?>[] {String.class}, directory));
        snapshot.getJSONObject("preferences").put("cloud_clipboard_relay", new JSONObject().put("url", url).put("token", token));
        long revision = snapshot.getLong("revision");
        snapshot.put("format_version", 1);
        value(call("savePreferences", new Class<?>[] {String.class, long.class, String.class},
            directory, revision, snapshot.toString()));
    }

    private static final String RELAY = "app.msime.android.CloudClipboardRelay";
    private static final String CODE_CHANGED = "relay_changed";
    private static final String STALE_BINDING = "relay:00000000000000000000000000000000";

    /** 设置页打开中继的那一步（`CloudClipboardRelay.open`），目录和设备名换成临时的。 */
    private static CloudClipboardApi.Backend openRelay(String directory, File keystore, String device) throws Exception {
        return (CloudClipboardApi.Backend) invoke(RELAY, "open", new Class<?>[] {String.class, String.class, String.class},
            directory, keystore.getAbsolutePath(), device);
    }

    /** 键盘后台线程上的选路（`CloudClipboardRelay.forPanel`）：null 是走账号。 */
    private static CloudClipboardApi.Backend forPanel(String directory, File keystore, boolean relay, String binding)
            throws Exception {
        return (CloudClipboardApi.Backend) invoke(RELAY, "forPanel",
            new Class<?>[] {String.class, String.class, String.class, boolean.class, String.class},
            directory, keystore.getAbsolutePath(), "Device A", relay, binding);
    }

    /** 键盘在主线程上按偏好快照记下的绑定（`CloudClipboardRelay.binding`）。 */
    private static String relayBinding(String directory) throws Exception {
        JSONObject preferences = value(call("loadPreferences", new Class<?>[] {String.class}, directory)).getJSONObject("preferences");
        return (String) invoke(RELAY, "binding", new Class<?>[] {JSONObject.class}, preferences);
    }

    private static BackendAccount.ClipboardPage clipboardPage(String json) throws Exception {
        return (BackendAccount.ClipboardPage) invoke(BackendAccount.class.getName(), "clipboardPage",
            new Class<?>[] {String.class}, json);
    }

    private static BackendAccount.ClipboardItem clipboardItem(String json) throws Exception {
        return (BackendAccount.ClipboardItem) invoke(BackendAccount.class.getName(), "clipboardItem",
            new Class<?>[] {String.class}, json);
    }

    /** 跑一步，返回它抛出的 {@link CloudApi.Failure} 的错误码；没抛出就失败。 */
    private static String failureCode(Callable<?> step) throws Exception {
        try {
            step.call();
        } catch (CloudApi.Failure failure) {
            check(failure.status == 0, "a relay failure carries no HTTP status");
            return failure.code;
        }
        throw new AssertionError("the step was expected to fail");
    }

    /**
     * 调应用包里的静态方法，包内可见的也行。`CloudClipboardRelay` 经 NativeClient 加载应用的 libmsime_android.so，不编进测试包（理由同 {@link CloudClipboardDriverDeviceSmoke}）；`BackendAccount` 编进了测试包，但那两个解析方法只在包内可见。方法抛出的异常原样抛出。
     */
    private static Object invoke(String type, String name, Class<?>[] types, Object... arguments) throws Exception {
        Method method = Class.forName(type).getDeclaredMethod(name, types);
        method.setAccessible(true);
        try {
            return method.invoke(null, arguments);
        } catch (InvocationTargetException thrown) {
            if (thrown.getCause() instanceof Exception cause) throw cause;
            throw thrown;
        }
    }

    private static String source(String directory) throws Exception {
        return call("cloudClipboardSource", new Class<?>[] {String.class}, directory);
    }

    private static String relay(String directory, File keystore, String device, JSONObject action) throws Exception {
        JSONObject request = new JSONObject()
            .put("preferences_directory", directory)
            .put("directory", keystore.getAbsolutePath())
            .put("device_name", device)
            .put("action", action);
        return call("relayCloudClipboard", new Class<?>[] {String.class}, request.toString());
    }

    /** NativeClient 从应用包里取，理由同 {@link CloudClipboardDriverDeviceSmoke}：它加载应用的 libmsime_android.so。 */
    private static String call(String name, Class<?>[] types, Object... arguments) throws Exception {
        Method method = Class.forName("app.msime.android.NativeClient").getMethod(name, types);
        return (String) method.invoke(null, arguments);
    }

    private static JSONObject value(String response) throws Exception {
        JSONObject root = new JSONObject(response);
        if (!root.optBoolean("ok")) throw new AssertionError("native call failed: " + root.opt("error"));
        return root.getJSONObject("value");
    }

    private static String error(String response) throws Exception {
        JSONObject root = new JSONObject(response);
        check(!root.optBoolean("ok"), "the call was expected to fail");
        return root.getString("error");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void delete(File file) {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) delete(child);
        if (file.exists() && !file.delete()) throw new IllegalStateException("could not delete " + file.getName());
    }

    /**
     * 本进程里的回环中继：每个连接答一个请求后关闭（`Connection: close`），请求交给假服务端；路径要带 {@link #PREFIX}、要带正确的静态令牌，否则答 401。置顶、删除、清空和保留时长这几个直接接口在这里答，其余交给假服务端。
     */
    private static final class Relay implements AutoCloseable {
        final CloudClipboardDriverDeviceSmoke.Server server = new CloudClipboardDriverDeviceSmoke.Server();
        private final ServerSocket socket;
        private final Thread thread;
        private int requests;
        private int badRequests;
        private int retention = -1;

        Relay() throws IOException {
            socket = new ServerSocket(0, 8, InetAddress.getByName("127.0.0.1"));
            thread = new Thread(this::serve, "cloud-clipboard-relay-smoke");
            thread.setDaemon(true);
            thread.start();
        }

        int port() { return socket.getLocalPort(); }

        synchronized int requests() { return requests; }

        synchronized int badRequests() { return badRequests; }

        synchronized int retention() { return retention; }

        /** 一个刚刚还开着、现在已经关掉的回环端口。 */
        int closedPort() throws IOException {
            try (ServerSocket probe = new ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))) {
                return probe.getLocalPort();
            }
        }

        private void serve() {
            while (!socket.isClosed()) {
                try (Socket client = socket.accept()) {
                    client.setSoTimeout(10_000);
                    answer(client);
                } catch (IOException closed) {
                    if (socket.isClosed()) return;
                } catch (Exception broken) {
                    // 假服务端读不懂的请求：连接关掉，客户端报 relay_unavailable，用例会失败并指出是哪一步。
                }
            }
        }

        private void answer(Socket client) throws Exception {
            InputStream input = new BufferedInputStream(client.getInputStream());
            String head = readHead(input);
            String[] lines = head.split("\r\n");
            String[] start = lines[0].split(" ");
            String method = start[0];
            String target = start[1];
            int length = 0;
            String authorization = "";
            for (int index = 1; index < lines.length; index++) {
                int colon = lines[index].indexOf(':');
                if (colon < 0) continue;
                String name = lines[index].substring(0, colon).trim().toLowerCase(Locale.ROOT);
                String value = lines[index].substring(colon + 1).trim();
                if (name.equals("content-length")) length = Integer.parseInt(value);
                if (name.equals("authorization")) authorization = value;
            }
            // InputStream#readNBytes 要 API 33，设备套件还要在 API 28 上跑。
            byte[] body = new byte[length];
            for (int read = 0; read < length; ) {
                int count = input.read(body, read, length - read);
                if (count < 0) throw new IOException("connection closed before the request body ended");
                read += count;
            }
            int status;
            String reply;
            synchronized (this) {
                requests++;
                if (!target.startsWith(PREFIX + API)) {
                    badRequests++;
                    status = 404;
                    reply = "";
                } else if (!("Bearer " + TOKEN).equals(authorization)) {
                    status = 401;
                    reply = "{\"error\":{\"code\":\"unauthorized\",\"message\":\"unauthorized\"}}";
                } else {
                    String path = target.substring(PREFIX.length());
                    String text = length == 0 ? null : new String(body, StandardCharsets.UTF_8);
                    CloudClipboardE2e.Reply answered = direct(method, path, text);
                    if (answered == null) answered = server.send(method, path, text);
                    status = answered.status();
                    reply = answered.body();
                }
            }
            byte[] payload = reply.getBytes(StandardCharsets.UTF_8);
            OutputStream output = client.getOutputStream();
            output.write(("HTTP/1.1 " + status + " X\r\nContent-Type: application/json\r\nContent-Length: "
                + payload.length + "\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
            output.write(payload);
            output.flush();
        }

        /** 中继专有的直接接口（置顶、删除、清空、保留时长）；不是这几个时为 null。在持锁时调用。 */
        private CloudClipboardE2e.Reply direct(String method, String path, String body) throws Exception {
            if ("DELETE".equals(method) && path.equals(API)) {
                server.items.clear();
                return new CloudClipboardE2e.Reply(204, "");
            }
            if ("PUT".equals(method) && path.equals(API + "/retention")) {
                retention = new JSONObject(body).getInt("days");
                return new CloudClipboardE2e.Reply(204, "");
            }
            if ("PUT".equals(method) && path.startsWith(API + "/") && path.endsWith("/pin")) {
                String id = path.substring((API + "/").length(), path.length() - "/pin".length());
                boolean pinned = new JSONObject(body).getBoolean("pinned");
                for (JSONObject item : server.items) if (id.equals(item.getString("id"))) item.put("pinned", pinned);
                return new CloudClipboardE2e.Reply(204, "");
            }
            if ("DELETE".equals(method) && path.startsWith(API + "/") && !path.startsWith(API + "/devices/")) {
                String id = path.substring((API + "/").length());
                List<JSONObject> kept = new ArrayList<>();
                for (JSONObject item : server.items) if (!id.equals(item.getString("id"))) kept.add(item);
                server.items.clear();
                server.items.addAll(kept);
                return new CloudClipboardE2e.Reply(204, "");
            }
            return null;
        }

        private static String readHead(InputStream input) throws IOException {
            ByteArrayOutputStream head = new ByteArrayOutputStream();
            int matched = 0;
            while (matched < 4) {
                int next = input.read();
                if (next < 0) throw new IOException("connection closed before the request head ended");
                head.write(next);
                matched = (next == '\r' && (matched == 0 || matched == 2)) || (next == '\n' && (matched == 1 || matched == 3))
                    ? matched + 1 : (next == '\r' ? 1 : 0);
                if (head.size() > 16 * 1024) throw new IOException("request head too large");
            }
            return head.toString(StandardCharsets.US_ASCII.name());
        }

        @Override public void close() throws IOException {
            socket.close();
            try {
                thread.join(5_000);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
            }
        }
    }
}
