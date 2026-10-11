package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.os.Looper;
import androidx.annotation.Nullable;
import app.msime.android.home.HostStore;
import java.io.File;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 本地语音模型，经共享层的 {@code msime_client_voice_local_model*} 列出、下载、取消和删除。
 *
 * <p>模型装在 {@code filesDir/voice-models/<id>}，与 HarmonyOS 和 Tauri 合包的位置相同；共享层给出的 {@code path} 就是 {@code voice_input.asr_model_path} 要写的值，键盘的本机识别器只认 {@code filesDir} 下的模型目录（{@link LocalAsrPolicy#installed}）。下载和资源包一样只在主进程的工作线程上跑，由 {@link ResourcePackService} 统一排队、出进度通知和取消。
 */
public final class LocalVoiceModels {
    /** 目录里的一个模型。{@code path} 是装好后的目录，没装时也给出，写进偏好前要先确认 {@code installed}。 */
    public record Model(String id, String title, String description, boolean streaming, boolean installed,
            String path, long archiveSize, long installedSize, String memory, boolean desktopOnly,
            List<ResourcePacks.ImportFile> importFiles) {}

    /** 安装进度：stage 是 "download"（从本地文件安装时是 "import"）、"verify"、"extract" 或 "done"。 */
    public interface Progress {
        void onProgress(String stage, long downloaded, long total);
    }

    private LocalVoiceModels() {}

    public static File root(File files) {
        return new File(files, "voice-models");
    }

    /** 目录里手机上可用的模型，顺序同目录。读不出来时抛 {@link ResourcePacks.Failure}。只读目录，但仍在工作线程上调用。 */
    public static List<Model> list(File files) throws ResourcePacks.Failure {
        try {
            JSONObject request = new JSONObject().put("root", root(files).getAbsolutePath());
            JSONArray models = value(NativeClient.voiceLocalModels(request.toString()))
                .getJSONObject("value").getJSONArray("models");
            ArrayList<Model> result = new ArrayList<>(models.length());
            for (int index = 0; index < models.length(); index++) {
                JSONObject model = models.getJSONObject(index);
                boolean installed = JsonPolicy.strictTrue(model.opt("installed"));
                boolean desktopOnly = JsonPolicy.strictTrue(model.opt("desktop_only"));
                if (!LocalVoiceModelPolicy.visible(desktopOnly, installed)) continue;
                result.add(new Model(model.getString("id"), JsonPolicy.strictStringOrEmpty(model.opt("title")),
                    JsonPolicy.strictStringOrEmpty(model.opt("description")),
                    JsonPolicy.strictTrue(model.opt("streaming")), installed,
                    JsonPolicy.strictStringOrEmpty(model.opt("path")), model.optLong("archive_size", 0),
                    model.optLong("installed_size", 0), JsonPolicy.strictStringOrEmpty(model.opt("memory")),
                    desktopOnly, ResourcePacks.importFiles(model.optJSONArray("import_files"))));
            }
            return ListPolicy.copyOrEmpty(result);
        } catch (JSONException error) {
            throw new ResourcePacks.Failure("invalid local model response");
        }
    }

    /**
     * 下载、校验并发布一个模型，返回装好的目录。阻塞到结束（几十到几百 MB），只在主进程的工作线程上调用。
     *
     * @param mirror 用户在 {@code voice_input.asr_model_mirror} 里填的下载前缀，没有时为空
     * @param files 不为 null 时不联网（空列表报缺文件），用这些本地文件（绝对路径，至多 16 个）安装，{@code mirror} 不用，进度阶段报 "import"
     */
    public static String install(Context context, String id, String mirror, @Nullable List<String> files,
            @Nullable Progress progress) throws ResourcePacks.Failure {
        requireInstallerThread(context);
        try {
            JSONObject request = new JSONObject()
                .put("root", root(context.getFilesDir()).getAbsolutePath())
                .put("id", id);
            if (TextPolicy.hasText(mirror)) request.put("mirror", TextPolicy.trimmed(mirror));
            if (files != null) request.put("files", new JSONArray(files));
            NativeClient.LocalModelProgress listener = progress == null ? null : json -> {
                try {
                    JSONObject event = new JSONObject(TextPolicy.utf8(json));
                    progress.onProgress(JsonPolicy.strictStringOrEmpty(event.opt("stage")),
                        event.optLong("downloaded", 0), event.optLong("total", 0));
                } catch (JSONException ignored) {
                    // 读不懂的一次进度只是少画一格，安装本身照常进行。
                }
            };
            return value(NativeClient.voiceLocalModelInstall(request.toString(), listener))
                .getJSONObject("value").getString("path");
        } catch (JSONException error) {
            throw new ResourcePacks.Failure("invalid local model response");
        }
    }

    /** 停下正在安装的 {@code id}；任何线程，立即返回。 */
    public static void cancel(String id) {
        if (id != null) NativeClient.voiceLocalModelCancel("{\"id\":" + JSONObject.quote(id) + "}");
    }

    /** 删除一个已装的模型。正在安装时共享层拒绝（{@code local_model_install_running}）。工作线程。 */
    public static void remove(Context context, String id) throws ResourcePacks.Failure {
        try {
            JSONObject request = new JSONObject()
                .put("root", root(context.getFilesDir()).getAbsolutePath())
                .put("id", id);
            value(NativeClient.voiceLocalModelRemove(request.toString()));
        } catch (JSONException error) {
            throw new ResourcePacks.Failure("invalid local model response");
        }
    }

    /** 下载完成后，还没有选用任何模型时选用这一个，和桌面设置页一致：离线识别和「本地模型」都读 {@code asr_model_path}。已经选了别的模型时不动。工作线程。 */
    public static void adoptIfUnset(Context context, String path) {
        JSONObject snapshot = HostStore.loadPreferences(context);
        JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
        if (preferences == null) return;
        try {
            JSONObject voice = preferences.optJSONObject("voice_input");
            if (voice == null) {
                voice = new JSONObject();
                preferences.put("voice_input", voice);
            }
            if (TextPolicy.hasText(JsonPolicy.strictStringOrEmpty(voice.opt("asr_model_path")))) return;
            voice.put("asr_model_path", path);
            HostStore.savePreferences(context, snapshot);
        } catch (JSONException ignored) {
            // 写不进去时用户在设置页手动选用，不影响已经装好的模型。
        }
    }

    private static void requireInstallerThread(Context context) {
        if (Looper.myLooper() == Looper.getMainLooper())
            throw new IllegalStateException("Local voice model install on the main thread");
        if (!context.getPackageName().equals(Application.getProcessName()))
            throw new IllegalStateException("Local voice model install outside the main process");
    }

    private static JSONObject value(String response) throws ResourcePacks.Failure, JSONException {
        JSONObject envelope = new JSONObject(response);
        if (!JsonPolicy.strictTrue(envelope.opt("ok")))
            throw new ResourcePacks.Failure(JsonPolicy.strictStringOrEmpty(envelope.opt("error")));
        return envelope;
    }
}
