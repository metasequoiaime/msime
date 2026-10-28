package app.msime.android.test;

import android.content.Context;
import android.system.Os;
import app.msime.android.DictionaryWebDavSync;
import app.msime.android.policy.DictionarySyncPolicy;
import java.io.File;
import java.lang.reflect.Method;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONObject;

/**
 * 由 {@link CloudClipboardRelayDeviceSmoke} 带 `-e dictionary-sync true` 时调用（设备测试包的 instrumentation 已经满 100 个，不再另加一个），在应用进程里经 JNI 跑「我的词条」网盘同步的入口 `msime_client_dictionary_sync`，以及设置页和键盘用的 Java 这一层（{@link DictionaryWebDavSync}）。
 *
 * <p>JVM 冒烟碰不到 Rust 的落盘和联网；这里验证 JNI 导出和请求形状、没配置时的 `dictionary_sync_not_configured`、不合规请求在联网前被拒、`local_change` 不联网、连不上的网盘是 `dictionary_sync_unavailable`（同步和测试连接都是），以及同步目录在应用的 noBackupFilesDir 下建得起、锁文件写得进、目录只有本应用能进（SELinux 和目录权限只有设备上才有）。网盘是一个没有服务在听的回环端口，不连外网；用的是临时的偏好目录和临时的同步目录，不碰应用自己的偏好和 `noBackupFilesDir/dictionary-sync`。完整的配对、互传和抢序号由 `crates/clipboard-relay/tests/dictionary_webdav_flow.rs` 对着真实的 WebDAV 服务端覆盖。
 */
final class DictionarySyncDeviceChecks {
    private DictionarySyncDeviceChecks() {}

    /** 跑全部检查，前后都清掉临时目录；失败时抛出。 */
    static void run(Context target) throws Exception {
        File scratch = new File(target.getCacheDir(), "dictionary-sync-smoke");
        File directory = new File(target.getNoBackupFilesDir(), "dictionary-sync-smoke");
        try {
            run(target, scratch, directory);
        } finally {
            delete(scratch);
            delete(directory);
        }
    }

    private static void run(Context target, File scratch, File directory) throws Exception {
        delete(scratch);
        delete(directory);
        File state = new File(scratch, "state");
        check(state.mkdirs(), "the scratch preferences directory is created");
        String preferences = state.getAbsolutePath();
        // loadPreferences 在空目录里给出出厂偏好；写一次，HostOptions 文档才能不带偏好、由 Rust 从目录里读。
        JSONObject snapshot = value(call("loadPreferences", new Class<?>[] {String.class}, preferences));
        save(preferences, snapshot);
        String options = new JSONObject()
            .put("api_version", 1)
            .put("resources", new File(scratch, "resources").getAbsolutePath())
            .put("user_data", new File(scratch, "user").getAbsolutePath())
            .put("cache", new File(scratch, "cache").getAbsolutePath())
            .put("dictionaries", new File(scratch, "dictionaries").getAbsolutePath())
            .put("preferences_directory", preferences)
            .toString();
        String path = directory.getAbsolutePath();

        // 没配置网盘：不联网，答 dictionary_sync_not_configured，也不建同步目录。
        check(DictionarySyncPolicy.CODE_NOT_CONFIGURED.equals(error(sync(options, path, status()))),
            "status without a WebDAV folder is dictionary_sync_not_configured");
        check(DictionarySyncPolicy.CODE_NOT_CONFIGURED.equals(error(sync(options, path, run("periodic")))),
            "run without a WebDAV folder is dictionary_sync_not_configured");
        check(!directory.exists(), "nothing is written before the WebDAV folder is configured");
        check(DictionarySyncPolicy.CODE_NOT_CONFIGURED.equals(failure(() -> bridge(options, path).status())),
            "the Java layer reports dictionary_sync_not_configured too");

        // 不合规的请求在联网前被拒。
        check(DictionarySyncPolicy.CODE_INVALID.equals(error(sync(options, path, new JSONObject().put("operation", "bogus")))),
            "an unknown operation is invalid_dictionary_sync");
        check(DictionarySyncPolicy.CODE_INVALID.equals(error(sync(options, "relative/dictionary-sync", status()))),
            "a relative directory is invalid_dictionary_sync");
        check(DictionarySyncPolicy.CODE_INVALID.equals(error(sync(options, path, run("hourly")))),
            "an unknown trigger is invalid_dictionary_sync");

        // 配置一个没有服务在听的回环网盘。
        int port = closedPort();
        String url = "http://127.0.0.1:" + port + "/dav/";
        snapshot = value(call("loadPreferences", new Class<?>[] {String.class}, preferences));
        snapshot.getJSONObject("preferences").put(DictionarySyncPolicy.PREFERENCE, new JSONObject()
            .put("url", url).put("username", "synthetic-user").put("password", "synthetic-app-password"));
        save(preferences, snapshot);

        JSONObject before = value(sync(options, path, status()));
        check(before.isNull("device_id") && !before.getBoolean("approved"), "a device that never reached the folder has no identity");

        // local_change 只记录、不联网；还没有设备身份时什么也不做。
        JSONObject recorded = value(sync(options, path, run(DictionarySyncPolicy.TRIGGER_LOCAL_CHANGE)));
        check(!recorded.getBoolean("network") && !recorded.has("error"), "local_change stays offline");

        // 定时同步要去登记，连不上是 dictionary_sync_unavailable；同步目录和锁文件建在 noBackupFilesDir 下，只有本应用能进。
        JSONObject unreachable = value(sync(options, path, run(DictionarySyncPolicy.TRIGGER_PERIODIC)));
        check(unreachable.getBoolean("network"), "a first periodic run goes to the folder to register");
        check(DictionarySyncPolicy.CODE_UNAVAILABLE.equals(unreachable.optString("error")),
            "an unreachable folder is dictionary_sync_unavailable, got " + unreachable.opt("error"));
        check(unreachable.getLong("next_check_ms") > 0, "the host is told when to try again");
        check(new File(directory, "dictionary-sync.lock").isFile(), "the lock file is written under noBackupFilesDir");
        check((Os.stat(path).st_mode & 0077) == 0, "the sync directory is private to the app");
        // 内部身份先持久化以记录失败请求；对外仍须等远端登记完成才公开。
        check(new File(directory, "cloud-clipboard-relay-keystore").isFile(),
            "the internal identity survives failed registration for request accounting");
        check((Os.stat(new File(directory, "cloud-clipboard-relay-keystore").getAbsolutePath()).st_mode & 0077) == 0,
            "the internal identity is private to the app");
        JSONObject stored = new JSONObject(new String(Files.readAllBytes(
            new File(directory, "dictionary-sync-state").toPath()), StandardCharsets.UTF_8));
        check(stored.getJSONObject("state").getJSONObject("schedule").getJSONArray("requests").length() > 0,
            "the failed HTTP request is charged in the persisted schedule");

        // 测试连接用草稿，连不上同样是 dictionary_sync_unavailable。
        JSONObject probe = new JSONObject().put("operation", "probe").put("url", url)
            .put("username", "synthetic-user").put("password", "synthetic-app-password");
        check(DictionarySyncPolicy.CODE_UNAVAILABLE.equals(error(sync(options, path, probe))),
            "probing an unreachable folder is dictionary_sync_unavailable");

        // 设置页和键盘经 Java 这一层走同样的路。
        DictionaryWebDavSync bridge = bridge(options, path);
        DictionaryWebDavSync.Status status = bridge.status();
        check(status.deviceId() == null && !status.approved(), "the Java layer reads the status");
        check(!DictionarySyncPolicy.imeMayRun(status.deviceId()), "the keyboard does not run before the settings page registers");
        DictionaryWebDavSync.Run periodic = bridge.run(DictionarySyncPolicy.TRIGGER_PERIODIC, false, false);
        check(!periodic.network() && periodic.error() == null && periodic.report() != null,
            "the next periodic call stays offline during the failure backoff");
        check(periodic.status().deviceId() == null && !DictionarySyncPolicy.imeMayRun(periodic.status().deviceId()),
            "an offline call does not expose the unregistered internal identity");
        check(DictionarySyncPolicy.CODE_UNAVAILABLE.equals(periodic.status().lastError()),
            "the offline status retains the previous failure");
        DictionaryWebDavSync.Run manual = bridge.run(DictionarySyncPolicy.TRIGGER_MANUAL, false, false);
        check(DictionarySyncPolicy.CODE_UNAVAILABLE.equals(manual.error()) && manual.report() == null,
            "the Java layer returns the sync failure without throwing");
        check(DictionarySyncPolicy.CODE_UNAVAILABLE.equals(failure(() -> {
            bridge.probe(url, "synthetic-user", "synthetic-app-password");
            return null;
        })), "the Java probe reports dictionary_sync_unavailable");
        check(DictionarySyncPolicy.CODE_UNAVAILABLE.equals(failure(() -> {
            bridge.trustRemote(url, "synthetic-user", "synthetic-app-password");
            return null;
        })), "the confirmed recovery reaches the same Java/JNI transport");

        // 两个进程用的目录：noBackupFilesDir/dictionary-sync。
        File shared = new File(target.getNoBackupFilesDir(), "dictionary-sync");
        check(shared.getAbsolutePath().equals(DictionaryWebDavSync.directory(target)),
            "the settings app and the keyboard share noBackupFilesDir/dictionary-sync");
    }

    private interface Work {
        Object call() throws Exception;
    }

    private static String failure(Work work) throws Exception {
        try {
            work.call();
        } catch (DictionaryWebDavSync.Failure failure) {
            return failure.code;
        }
        throw new AssertionError("the call was expected to fail");
    }

    private static DictionaryWebDavSync bridge(String options, String directory) throws Exception {
        return DictionaryWebDavSync.open(options, directory, "Synthetic Device");
    }

    private static JSONObject status() throws Exception {
        return new JSONObject().put("operation", "status");
    }

    private static JSONObject run(String trigger) throws Exception {
        return new JSONObject().put("operation", "run").put("trigger", trigger);
    }

    private static String sync(String options, String directory, JSONObject action) throws Exception {
        JSONObject request = new JSONObject()
            .put("options", new JSONObject(options))
            .put("directory", directory)
            .put("device_name", "Synthetic Device")
            .put("action", action);
        return call("dictionarySync", new Class<?>[] {String.class}, request.toString());
    }

    private static void save(String directory, JSONObject snapshot) throws Exception {
        long revision = snapshot.getLong("revision");
        snapshot.put("format_version", 1);
        value(call("savePreferences", new Class<?>[] {String.class, long.class, String.class},
            directory, revision, snapshot.toString()));
    }

    private static int closedPort() throws Exception {
        try (ServerSocket socket = new ServerSocket(0, 1, InetAddress.getByName("127.0.0.1"))) {
            return socket.getLocalPort();
        }
    }

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
        if (file == null || !file.exists()) return;
        File[] children = file.listFiles();
        if (children != null) for (File child : children) delete(child);
        if (!file.delete() && file.exists()) throw new IllegalStateException("could not delete " + file.getName());
    }
}
