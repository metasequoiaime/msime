package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.os.Bundle;
import app.msime.android.CloudClipboardE2e;
import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Method;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 在应用进程里经 JNI 跑云剪贴板的自建中继（`msime_client_cloud_clipboard_source` 与 `msime_client_relay_cloud_clipboard`）：对面是本进程里一台只说 HTTP/1.1 的回环中继，背后是驱动冒烟里那个按协议写的假服务端（{@link CloudClipboardDriverDeviceSmoke.Server}）。
 *
 * <p>JVM 冒烟碰不到 Rust 的联网和落盘；这里验证 JNI 导出、请求形状、路径前缀、静态令牌、401 报成 `relay_unauthorized`（不是账号未登录），以及 Rust 在应用的 noBackupFilesDir 下建得起密钥串目录（SELinux 和目录权限只有设备上才有）。用的是临时的偏好目录和临时的密钥串目录，不碰应用自己的偏好、账号会话和密钥串，也不连外网。
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
        try (Relay relay = new Relay()) {
            run(getTargetContext(), relay, scratch, keysA, keysB);
            result.putString("stream",
                "MSIME_DEVICE_SMOKE_PASSED: cloud clipboard relay over JNI: source, prefix, bearer token, register, approve, encrypted add, list, pin, delete, unauthorized and unavailable\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError | LinkageError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: cloud clipboard relay ("
                + error.getClass().getSimpleName() + ": " + error.getMessage() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        } finally {
            delete(scratch);
            delete(keysA);
            delete(keysB);
        }
    }

    private static void run(Context context, Relay relay, File scratch, File keysA, File keysB) throws Exception {
        delete(scratch);
        File preferences = new File(scratch, "state");
        check(preferences.mkdirs(), "the scratch preferences directory is created");
        String directory = preferences.getAbsolutePath();

        // 没配置中继：走水杉账号，中继入口直接答 relay_not_configured，不联网。
        check("account".equals(value(source(directory)).getString("source")), "no relay means the account");
        check("relay_not_configured".equals(error(relay(directory, keysA, "Device A", list()))),
            "the relay entry refuses without a relay");

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

        // 令牌被拒是 relay_unauthorized，不会被当成水杉账号未登录。
        configure(directory, url, "synthetic-relay-token-0002");
        check("relay_unauthorized".equals(error(relay(directory, keysA, "Device A", list()))),
            "a rejected token is relay_unauthorized");
        // 没有服务的回环端口：连不上是 relay_unavailable。
        configure(directory, "http://127.0.0.1:" + relay.closedPort() + PREFIX, TOKEN);
        check("relay_unavailable".equals(error(relay(directory, keysA, "Device A", list()))),
            "an unreachable relay is relay_unavailable");
        check("invalid_cloud_clipboard".equals(error(relay(directory, keysA, "Device A",
            new JSONObject().put("operation", "pin").put("id", "short").put("pinned", true)))),
            "a malformed action is refused before the network");
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
     * 本进程里的回环中继：每个连接答一个请求后关闭（`Connection: close`），请求交给假服务端；路径要带 {@link #PREFIX}、要带正确的静态令牌，否则答 401。置顶、删除和清空这三个直接接口在这里答，其余交给假服务端。
     */
    private static final class Relay implements AutoCloseable {
        final CloudClipboardDriverDeviceSmoke.Server server = new CloudClipboardDriverDeviceSmoke.Server();
        private final ServerSocket socket;
        private final Thread thread;
        private int requests;
        private int badRequests;

        Relay() throws IOException {
            socket = new ServerSocket(0, 8, InetAddress.getByName("127.0.0.1"));
            thread = new Thread(this::serve, "cloud-clipboard-relay-smoke");
            thread.setDaemon(true);
            thread.start();
        }

        int port() { return socket.getLocalPort(); }

        synchronized int requests() { return requests; }

        synchronized int badRequests() { return badRequests; }

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

        /** 中继专有的直接接口；不是这几个时为 null。 */
        private CloudClipboardE2e.Reply direct(String method, String path, String body) throws Exception {
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
