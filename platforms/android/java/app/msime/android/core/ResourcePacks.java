package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.os.Looper;
import java.io.File;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 按需下载的资源包（日文词典、语言词库、离线释义、语音运行库），经共享层的 {@code msime_client_resource_pack_*} 列出、下载和收编。
 *
 * <p>资源包装在 {@code filesDir/bootstrap/state/resource-packs/<id>/}：{@code bootstrap/state} 就是 host-api 的 {@code preferences_directory}，会话在获得焦点时从那里找到新装好的资源包；Bootstrap 随安装包替换的目录碰不到它。设置页和键盘的方案可用判断都以这里的 {@link #installed} 为准。
 *
 * <p>下载只在主进程里跑，同一个资源包同一时间只有一个安装（共享层另有 {@code resource_pack_busy} 保证）。:ime 进程只读状态和已发布的文件，从不安装或收编：两个进程同时装同一个包会互相清掉对方的暂存目录。
 */
public final class ResourcePacks {
    public static final String JAPANESE = "japanese";
    public static final String LANGUAGE_DICTIONARIES = "language-dictionaries";
    public static final String OFFLINE_GLOSSES = "offline-glosses";
    public static final String VOICE_RUNTIME = "voice-runtime";

    /** 一个资源包的状态。{@code state} 是 "missing"、"installed" 或 "outdated"；{@code size} 是下载字节数；{@code schemes} 是它提供数据的输入方案 id。 */
    public record Pack(String id, String state, long size, List<String> schemes) {
        public boolean installed() {
            return "installed".equals(state);
        }
    }

    /** 共享层报告的失败；{@link #code()} 是冒号之前的错误码，例如 {@code local_model_network}、{@code local_model_cancelled}、{@code resource_pack_busy}。 */
    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        private final String code;

        Failure(String error) {
            super(error == null || error.isEmpty() ? "resource_pack_failed" : error);
            String message = getMessage();
            int separator = message.indexOf(':');
            this.code = separator < 0 ? message : message.substring(0, separator);
        }

        public String code() {
            return code;
        }

        public boolean cancelled() {
            return "local_model_cancelled".equals(code);
        }
    }

    private ResourcePacks() {}

    /** {@code filesDir/bootstrap/state}，也就是传给共享层的 state_root。 */
    public static File stateRoot(File files) {
        return new File(files, "bootstrap/state");
    }

    /** 全部资源包的状态，顺序同共享层。只读几个文件属性，但仍是一次 JNI 调用，不要在逐键路径上反复调用。读不出来时抛 {@link Failure}。 */
    public static List<Pack> list(File files) throws Failure {
        try {
            JSONObject request = new JSONObject().put("state_root", stateRoot(files).getAbsolutePath());
            JSONArray packs = value(NativeClient.resourcePacks(request.toString())).getJSONArray("value");
            ArrayList<Pack> result = new ArrayList<>(packs.length());
            for (int index = 0; index < packs.length(); index++) {
                JSONObject pack = packs.getJSONObject(index);
                JSONArray schemes = pack.optJSONArray("schemes");
                ArrayList<String> ids = new ArrayList<>(schemes == null ? 0 : schemes.length());
                for (int scheme = 0; schemes != null && scheme < schemes.length(); scheme++)
                    ids.add(schemes.getString(scheme));
                result.add(new Pack(pack.getString("id"), pack.getString("state"),
                    pack.optLong("size", 0), List.copyOf(ids)));
            }
            return List.copyOf(result);
        } catch (JSONException error) {
            throw new Failure("invalid resource pack response");
        }
    }

    public static List<Pack> list(Context context) throws Failure {
        return list(context.getFilesDir());
    }

    /** 某个资源包的状态；不认识的 id 或读不出状态时为 null。 */
    public static Pack pack(File files, String id) {
        try {
            for (Pack pack : list(files)) if (pack.id().equals(id)) return pack;
        } catch (Failure error) {
            android.util.Log.w("MSIMEResourcePacks", "Resource pack state unavailable: " + error.code());
        }
        return null;
    }

    /** 已完整安装的资源包 id，一次 JNI 列出就得到全部，键盘在 onStartInput 里用它而不是逐个问 {@link #installed}。读不出状态时为空集合。 */
    public static java.util.Set<String> installedIds(Context context) {
        java.util.Set<String> ids = new java.util.HashSet<>();
        try {
            for (Pack pack : list(context)) if (pack.installed()) ids.add(pack.id());
        } catch (Failure error) {
            android.util.Log.w("MSIMEResourcePacks", "Resource pack state unavailable: " + error.code());
        }
        return java.util.Set.copyOf(ids);
    }

    /** 资源包已完整安装且与编译进来的锁一致。读不出状态时按未安装处理。 */
    public static boolean installed(Context context, String id) {
        return installed(context.getFilesDir(), id);
    }

    public static boolean installed(File files, String id) {
        Pack pack = pack(files, id);
        return pack != null && pack.installed();
    }

    /** 已安装资源包的目录；没装好时为 null。目录里的文件只读不写：日文词典被输入法内存映射着，更新时共享层整体换目录，从不原地改写。 */
    public static File directory(File files, String id) {
        if (!installed(files, id)) return null;
        File directory = new File(stateRoot(files), "resource-packs/" + id);
        return Files.isDirectory(directory.toPath(), LinkOption.NOFOLLOW_LINKS) ? directory : null;
    }

    /**
     * 下载、校验并发布一个资源包，返回发布后的目录。
     *
     * <p>阻塞到结束（几十 MB 的下载），只能在主进程的工作线程上调用。{@code sources} 是用户自填的镜像前缀（沿用 {@code voice_input.asr_model_mirror}），按顺序先试，然后是项目镜像，最后是锁文件里的原地址；完整性只认编译进来的长度和 SHA-256。没下完的文件留着，下次用 HTTP Range 续传。{@link #cancel} 让它以 {@link Failure#cancelled()} 结束。
     */
    public static File install(Context context, String id, List<String> sources,
            NativeClient.ResourcePackProgress progress) throws Failure {
        requireInstallerThread(context);
        try {
            JSONObject request = new JSONObject()
                .put("state_root", stateRoot(context.getFilesDir()).getAbsolutePath())
                .put("pack", id);
            JSONArray mirrors = new JSONArray();
            if (sources != null) {
                for (String source : sources) {
                    if (source != null && !source.trim().isEmpty()) mirrors.put(source.trim());
                }
            }
            if (mirrors.length() > 0) request.put("sources", mirrors);
            return new File(value(NativeClient.resourcePackInstall(request.toString(), progress))
                .getJSONObject("value").getString("path"));
        } catch (JSONException error) {
            throw new Failure("invalid resource pack response");
        }
    }

    /** 停下正在安装的 {@code id}；任何线程，立即返回。 */
    public static void cancel(String id) {
        if (id != null) NativeClient.resourcePackCancel(id);
    }

    /**
     * 把 {@code source} 里属于资源包 {@code id} 的文件收编为已安装的资源包，不重新下载：同一文件系统内改名移走（不复制），按编译进来的锁核对长度和 SHA-256 后发布。失败时文件原样放回 {@code source}。同一组字节已经装好时直接返回，{@code source} 不动。
     *
     * <p>只给 Bootstrap 在主进程升级时用；要哈希整组文件，在工作线程上调用。
     */
    static File adopt(File files, String id, File source) throws Failure {
        try {
            JSONObject request = new JSONObject()
                .put("state_root", stateRoot(files).getAbsolutePath())
                .put("pack", id)
                .put("source", source.getAbsolutePath());
            return new File(value(NativeClient.resourcePackAdopt(request.toString()))
                .getJSONObject("value").getString("path"));
        } catch (JSONException error) {
            throw new Failure("invalid resource pack response");
        }
    }

    /** 安装只在主进程的工作线程上：UI 线程会卡住，:ime 进程按约定只读。 */
    private static void requireInstallerThread(Context context) {
        if (Looper.myLooper() == Looper.getMainLooper())
            throw new IllegalStateException("Resource pack install on the main thread");
        if (!context.getPackageName().equals(Application.getProcessName()))
            throw new IllegalStateException("Resource pack install outside the main process");
    }

    private static JSONObject value(String response) throws Failure, JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!JsonPolicy.strictTrue(envelope.opt("ok")))
            throw new Failure(JsonPolicy.strictStringOrEmpty(envelope.opt("error")));
        return envelope;
    }
}
