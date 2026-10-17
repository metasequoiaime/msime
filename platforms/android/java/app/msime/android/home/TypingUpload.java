package app.msime.android.home;

import android.content.Context;
import android.util.Log;
import androidx.annotation.Nullable;
import androidx.annotation.WorkerThread;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import app.msime.android.JsonPolicy;
import app.msime.android.SyncSwitch;
import app.msime.android.ThreadPolicy;
import app.msime.android.TypingUploadApi;
import java.time.LocalDate;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 打字统计的「上传到云端」：开关、上次上传的时间和设备标识存在共享层（`msime_client_typing_statistics` 的 `typing-statistics-upload.json`），请求体和十分钟的节流也在那里，这里只带着真实账号的令牌把请求发出去。
 *
 * <p>共享层要求 `upload_request`、发送、`upload_succeeded` 和关闭后的 `DELETE` 排在同一个串行队列里，免得关掉以后的删除和还在路上的上传交错，所以全部经过 {@link #WORKER}。回到前台（{@link HomeActivity}）和打开打字统计页时各试一次，节流由共享层判断；只有真实账号上传，匿名账号和没登录时什么都不发。
 */
final class TypingUpload {
    private static final String TAG = "MSIMETypingUpload";
    private static final String PLATFORM = "android";
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-typing-upload"));

    /** 开关、上次成功上传的时间（还没有时为 null）和这台设备有没有登录真实账号。 */
    record Status(boolean enabled, @Nullable Long lastUploadMs, boolean signedIn) {}

    /** 打开或关闭的结果：新的状态，以及要告诉用户的一句话。 */
    record Change(Status status, String message) {}

    private TypingUpload() {}

    /** 回到前台时调用：开着上传、登录着真实账号时试一次，不到十分钟由共享层跳过。不阻塞调用方。 */
    static void onResume(Context context) {
        Context application = context.getApplicationContext();
        WORKER.execute(() -> {
            try {
                upload(application, false);
            } catch (CloudApi.Failure failure) {
                Log.w(TAG, "typing statistics upload failed: " + failure.status + " " + failure.code);
            } catch (RuntimeException unexpected) {
                Log.w(TAG, "typing statistics upload failed", unexpected);
            }
        });
    }

    /** 读开关状态，顺带补传一次（与共享设置页挂上时调 `sync` 相同）。补传失败不打扰用户。在工作线程调用。 */
    @WorkerThread static Status sync(Context context) {
        return serial(() -> {
            try {
                upload(context, false);
            } catch (CloudApi.Failure failure) {
                Log.w(TAG, "typing statistics upload failed: " + failure.status + " " + failure.code);
            }
            return status(context);
        });
    }

    /**
     * 打开或关闭上传。打开要求已登录真实账号，打开后立刻上传一次；关闭先在本机关掉，再删除云端的数据，删除失败时开关仍是关的。
     *
     * @return 新的状态和要提示的话；共享层读写失败时为 null
     */
    @WorkerThread @Nullable static Change setEnabled(Context context, boolean enabled) {
        return serial(() -> {
            boolean signedIn = signedIn(context);
            if (enabled && !signedIn) return new Change(status(context), "先在「我的」页登录水杉账号，再打开上传。");
            if (action(context, "set_upload_enabled", enabled) == null) return null;
            String message;
            if (enabled) {
                try {
                    upload(context, true);
                    message = "已打开上传，最近 7 天的每日计数已传到你的水杉账号。";
                } catch (CloudApi.Failure failure) {
                    message = TypingUploadApi.uploadMessage(failure, true);
                }
            } else if (!signedIn) {
                message = "已关闭上传。没有登录，云端的数据没有删掉，登录后打开再关闭一次即可删除。";
            } else {
                try {
                    new TypingUploadApi(new CloudApi(context)).deleteAll();
                    message = "已关闭上传，云端的打字统计已删除。";
                } catch (CloudApi.Failure failure) {
                    message = TypingUploadApi.uploadMessage(failure, false);
                }
            }
            return new Change(status(context), message);
        });
    }

    /** 只读状态，不联网。在工作线程调用。 */
    @WorkerThread static Status status(Context context) {
        JSONObject value = action(context, "upload_status", null);
        boolean enabled = value != null && JsonPolicy.strictTrue(value.opt("enabled"));
        Long last = value == null ? null : JsonPolicy.strictLong(value.opt("lastUploadMs"));
        return new Status(enabled, last, signedIn(context));
    }

    /** 只有真实账号上传：匿名账号的数据不该跟着设备走，也没有群友能绑定它。 */
    private static boolean signedIn(Context context) {
        return new BackendAccount(context).signedIn() && SyncSwitch.validLoginKind(SyncSwitch.loginKind(context));
    }

    /** 在 {@link #WORKER} 上做一次：要发时发出去，成功后告诉共享层。没开、不到时候、没有字数或没登录时什么都不做。 */
    private static void upload(Context context, boolean force) throws CloudApi.Failure {
        if (!signedIn(context)) return;
        JSONObject request = new JSONObject();
        try {
            request.put("operation", "upload_request").put("day", LocalDate.now().toString())
                .put("platform", PLATFORM).put("force", force);
        } catch (JSONException impossible) {
            return;
        }
        JSONObject value = HostStore.statisticsAction(context, request);
        if (value == null || value.has("skip")) return;
        Object path = value.opt("path");
        Object device = value.opt("device");
        JSONObject body = value.optJSONObject("body");
        if (!(path instanceof String target) || !(device instanceof String id) || body == null
                || !"PUT".equals(value.opt("method"))) {
            Log.w(TAG, "typing statistics upload request is malformed");
            return;
        }
        new TypingUploadApi(new CloudApi(context)).upload(target, body);
        JSONObject succeeded = new JSONObject();
        try {
            succeeded.put("operation", "upload_succeeded").put("device", id);
        } catch (JSONException impossible) {
            return;
        }
        HostStore.statisticsAction(context, succeeded);
    }

    @Nullable private static JSONObject action(Context context, String operation, @Nullable Boolean enabled) {
        JSONObject action = new JSONObject();
        try {
            action.put("operation", operation);
            if (enabled != null) action.put("enabled", enabled.booleanValue());
        } catch (JSONException impossible) {
            return null;
        }
        return HostStore.statisticsAction(context, action);
    }

    /** 在串行队列上做完再返回；调用方本身就在工作线程上。 */
    private static <T> T serial(Callable<T> work) {
        try {
            return WORKER.submit(work).get();
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException(interrupted);
        } catch (ExecutionException failed) {
            Throwable cause = failed.getCause();
            if (cause instanceof RuntimeException runtime) throw runtime;
            throw new IllegalStateException(cause);
        }
    }
}
