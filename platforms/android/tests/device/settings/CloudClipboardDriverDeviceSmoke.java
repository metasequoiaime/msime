package app.msime.android.test;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import app.msime.android.CloudClipboardE2e;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 在应用进程里经 JNI 跑真正的共享驱动（`msime_client_cloud_clipboard_e2e`），对面是内存里按 `docs/cloud-clipboard-e2e.md` 写的假服务端：两台设备登记、建钥、加密上传、解密列表、待批准、允许、移除后重新待批准。
 *
 * <p>JVM 冒烟只能用假驱动；这里验证的是 JNI 导出、驱动的输入输出形状和宿主循环在设备上真的接得上，内容在服务端只有密文。不联网，不碰应用的密钥串文件和账号会话。
 */
public final class CloudClipboardDriverDeviceSmoke extends Instrumentation {
    private static final String API = "/v1/users/me/clipboard";
    private static final String TEXT = "合成的云剪贴板文本 synthetic";

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            run();
            result.putString("stream",
                "MSIME_DEVICE_SMOKE_PASSED: cloud clipboard e2e driver over JNI: register, key, encrypted add, list, pending, approve and remove\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError | LinkageError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: cloud clipboard e2e driver ("
                + error.getClass().getSimpleName() + ": " + error.getMessage() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }

    private static void run() throws Exception {
        Server server = new Server();
        Memory keysA = new Memory();
        Memory keysB = new Memory();
        // 驱动从应用包里取：它经 NativeClient 加载应用的 libmsime_android.so，把 NativeClient 连同它引用的一串类编进测试包不值得。测试包和应用包在同一个类加载器里（测试包在前），取到的驱动与这里的 CloudClipboardE2e 是同一个类。
        CloudClipboardE2e.Driver driver = (CloudClipboardE2e.Driver) Class.forName("app.msime.android.CloudClipboardNativeDriver")
            .getField("INSTANCE").get(null);
        CloudClipboardE2e.Local deviceA = new CloudClipboardE2e.Local(driver, keysA, "Device A");
        CloudClipboardE2e.Local deviceB = new CloudClipboardE2e.Local(driver, keysB, "Device B");

        JSONObject status = new JSONObject(CloudClipboardE2e.run(deviceA, CloudClipboardE2e.Operation.status(), server, null));
        check("ready".equals(status.getString("status")), "the first device creates the key");
        check(status.getJSONArray("devices").length() == 1 && status.getJSONArray("devices").getJSONObject(0).getBoolean("current"),
            "the first device is listed as this device");
        check(status.getString("code").matches("\\d{4} \\d{4} \\d{4} \\d{4}"), "the verification code has four groups");
        check(keysA.value != null && keysA.value.split("\\.").length == 4, "the keystore now holds the clipboard key");
        String idA = status.getString("device_id");

        JSONObject added = new JSONObject(CloudClipboardE2e.run(deviceA, CloudClipboardE2e.Operation.add(TEXT), server, null));
        check(TEXT.equals(added.getString("text")), "the uploaded item comes back decrypted");
        String ciphertext = server.items.get(0).getString("ciphertext");
        check(ciphertext.startsWith("v1.") && !ciphertext.contains("synthetic"), "the server holds only ciphertext");
        JSONObject listA = new JSONObject(CloudClipboardE2e.run(deviceA, CloudClipboardE2e.Operation.list("SYNTH"), server, null));
        check(listA.getJSONArray("items").length() == 1 && TEXT.equals(listA.getJSONArray("items").getJSONObject(0).getString("text")),
            "the list is decrypted and searched on the device");
        JSONObject missed = new JSONObject(CloudClipboardE2e.run(deviceA, CloudClipboardE2e.Operation.list("absent"), server, null));
        check(missed.getJSONArray("items").length() == 0, "a search that matches nothing lists nothing");

        JSONObject listB = new JSONObject(CloudClipboardE2e.run(deviceB, CloudClipboardE2e.Operation.list(""), server, null));
        check("pending".equals(listB.getJSONObject("e2e").getString("status")), "a second device waits for approval");
        check(listB.getJSONArray("items").length() == 0, "a waiting device reads nothing");
        String codeB = listB.getJSONObject("e2e").getString("code");
        try {
            CloudClipboardE2e.run(deviceB, CloudClipboardE2e.Operation.add(TEXT), server, null);
            throw new AssertionError("a waiting device must not upload");
        } catch (CloudClipboardE2e.Failure failure) {
            check(failure.status == 0 && "clipboard_device_pending".equals(failure.code), "the pending code is kept");
        }

        JSONObject seen = new JSONObject(CloudClipboardE2e.run(deviceA, CloudClipboardE2e.Operation.status(), server, null));
        JSONObject pending = null;
        JSONArray devices = seen.getJSONArray("devices");
        for (int index = 0; index < devices.length(); index++) {
            JSONObject device = devices.getJSONObject(index);
            if (!device.getBoolean("current")) pending = device;
        }
        check(pending != null && !pending.getBoolean("approved"), "the approver sees the waiting device");
        check(codeB.equals(pending.getString("code")), "both devices show the same verification code");
        check("Device B".equals(pending.getString("name")), "the waiting device carries its name");

        JSONObject approved = new JSONObject(CloudClipboardE2e.run(deviceA,
            CloudClipboardE2e.Operation.approve(pending.getString("device_id")), server, null));
        check(approved.getJSONArray("devices").length() == 2, "both devices are listed after approval");
        JSONObject readB = new JSONObject(CloudClipboardE2e.run(deviceB, CloudClipboardE2e.Operation.list(""), server, null));
        check("ready".equals(readB.getJSONObject("e2e").getString("status"))
            && TEXT.equals(readB.getJSONArray("items").getJSONObject(0).getString("text")),
            "the approved device decrypts the first device's item");

        String keyBefore = server.keyId;
        CloudClipboardE2e.run(deviceA, CloudClipboardE2e.Operation.remove(pending.getString("device_id")), server, null);
        check(!keyBefore.equals(server.keyId) && server.items.isEmpty(), "removing a device rotates the key and clears the items");
        check(server.wraps.size() == 1 && server.wraps.containsKey(idA), "the new key is wrapped for this device only");
        JSONObject afterRemove = new JSONObject(CloudClipboardE2e.run(deviceB, CloudClipboardE2e.Operation.list(""), server, null));
        check("pending".equals(afterRemove.getJSONObject("e2e").getString("status")), "a removed device must be approved again");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    /** 内存里的密钥串：一个进程、一台设备，不碰应用的文件。 */
    private static final class Memory implements CloudClipboardE2e.Keystore {
        String value;

        @Override public String load() { return value; }

        @Override public void save(String next, CloudClipboardE2e.Guard guard) throws Exception {
            if (guard != null) guard.check();
            value = next;
        }

        @Override public void delete() { value = null; }
    }

    /** 按协议文档写的假服务端，只做驱动用得到的几个接口。中继的设备冒烟（{@link CloudClipboardRelayDeviceSmoke}）把它放在回环 HTTP 后面再用一次。 */
    static final class Server implements CloudClipboardE2e.Transport {
        final Map<String, JSONObject> devices = new LinkedHashMap<>();
        final Map<String, String> wraps = new LinkedHashMap<>();
        final List<JSONObject> items = new ArrayList<>();
        String keyId;
        private int nextItem;

        @Override public CloudClipboardE2e.Reply send(String method, String path, String body) throws Exception {
            JSONObject request = body == null ? null : new JSONObject(body);
            if ("GET".equals(method) && path.startsWith(API + "/keys?device_id=")) {
                String id = path.substring((API + "/keys?device_id=").length());
                JSONArray list = new JSONArray();
                for (JSONObject device : devices.values()) {
                    list.put(new JSONObject(device.toString()).put("approved", wraps.containsKey(device.getString("device_id"))));
                }
                return ok(new JSONObject().put("enabled", true).put("key_id", keyId == null ? JSONObject.NULL : keyId)
                    .put("wrap", wraps.containsKey(id) ? wraps.get(id) : JSONObject.NULL).put("devices", list));
            }
            if ("PUT".equals(method) && path.startsWith(API + "/devices/")) {
                String id = path.substring((API + "/devices/").length());
                devices.put(id, new JSONObject().put("device_id", id).put("public_key", request.getString("public_key"))
                    .put("name", request.getString("name")).put("created_at", "2026-10-11T08:00:00Z"));
                return new CloudClipboardE2e.Reply(204, "");
            }
            if ("DELETE".equals(method) && path.startsWith(API + "/devices/")) {
                String id = path.substring((API + "/devices/").length());
                wraps.remove(id);
                return devices.remove(id) == null ? error(404, "not_found") : new CloudClipboardE2e.Reply(204, "");
            }
            if ("POST".equals(method) && path.equals(API + "/keys")) {
                Object previous = request.opt("previous_key_id");
                String expected = previous == null || previous == JSONObject.NULL ? null : (String) previous;
                if (!java.util.Objects.equals(expected, keyId)) return error(409, "clipboard_key_conflict");
                JSONArray revoke = request.getJSONArray("revoke");
                for (int index = 0; index < revoke.length(); index++) devices.remove(revoke.getString(index));
                items.clear();
                wraps.clear();
                keyId = request.getString("key_id");
                JSONArray newWraps = request.getJSONArray("wraps");
                for (int index = 0; index < newWraps.length(); index++) {
                    JSONObject wrap = newWraps.getJSONObject(index);
                    wraps.put(wrap.getString("device_id"), wrap.getString("wrap"));
                }
                return new CloudClipboardE2e.Reply(204, "");
            }
            if ("PUT".equals(method) && path.startsWith(API + "/keys/")) {
                String[] parts = path.substring((API + "/keys/").length()).split("/");
                if (!parts[0].equals(keyId)) return error(409, "clipboard_key_conflict");
                if (!devices.containsKey(parts[2])) return error(404, "not_found");
                wraps.put(parts[2], request.getString("wrap"));
                return new CloudClipboardE2e.Reply(204, "");
            }
            if ("GET".equals(method) && path.equals(API)) {
                JSONArray list = new JSONArray();
                for (JSONObject item : items) list.put(item);
                return ok(new JSONObject().put("enabled", true).put("retention_days", 0)
                    .put("key_id", keyId == null ? JSONObject.NULL : keyId).put("items", list));
            }
            if ("POST".equals(method) && path.equals(API)) {
                if (request.has("text")) return error(400, "clipboard_e2e_required");
                if (!request.getString("key_id").equals(keyId)) return error(409, "clipboard_key_conflict");
                String id = String.format(java.util.Locale.ROOT, "%064x", ++nextItem);
                JSONObject item = new JSONObject().put("id", id).put("ciphertext", request.getString("ciphertext"))
                    .put("key_id", keyId).put("updated_at", "2026-10-11T08:00:00Z").put("pinned", false).put("device", "Device A");
                items.add(0, item);
                return ok(item);
            }
            return error(404, "not_found");
        }

        private static CloudClipboardE2e.Reply ok(JSONObject body) {
            return new CloudClipboardE2e.Reply(200, body.toString());
        }

        private static CloudClipboardE2e.Reply error(int status, String code) throws Exception {
            return new CloudClipboardE2e.Reply(status,
                new JSONObject().put("error", new JSONObject().put("code", code).put("message", code)).toString());
        }
    }
}
