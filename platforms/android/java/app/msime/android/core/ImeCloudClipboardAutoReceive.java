package app.msime.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import app.msime.android.clipboard.CloudClipboardReceivePolicy;
import java.util.Objects;
import java.util.concurrent.RejectedExecutionException;
import org.json.JSONObject;

/** 输入视图可见期间收取新文本；账号校验和网络请求仍由既有加密驱动负责。 */
final class ImeCloudClipboardAutoReceive {
    private static final String LABEL = "水杉云剪贴板";
    private final MSIMEInputService service;
    private final Runnable poll = this::poll;
    private CloudClipboardReceivePolicy policy = new CloudClipboardReceivePolicy();
    private ImeCloudClipboardAutoSync.Binding owner;
    private volatile boolean active;
    private volatile long lifecycleGeneration;
    private boolean busy;

    ImeCloudClipboardAutoReceive(MSIMEInputService service) {
        this.service = service;
    }

    private boolean allowed() {
        return service.imePanels.cloudClipboardAllowed()
            && service.clipboardMode != ClipboardModePolicy.Mode.VISITOR;
    }

    /** 中继绑定只看内存里的偏好快照，主线程上可以调用。 */
    private String relay() {
        JSONObject preferences = service.preferencesSnapshot == null
            ? null : service.preferencesSnapshot.optJSONObject("preferences");
        return CloudClipboardRelay.binding(preferences);
    }

    /** 账号绑定要同步调用主进程的 provider，主进程被回收时会把它冷启动，所以只能在工作线程上解析，不能放在键盘界面线程。 */
    private static ImeCloudClipboardAutoSync.Binding resolve(Context context, String relay) {
        if (!relay.isEmpty()) return new ImeCloudClipboardAutoSync.Binding(true, relay, 0L);
        SyncSwitch.Binding binding = SyncSignals.binding(context);
        return binding == null ? null : new ImeCloudClipboardAutoSync.Binding(
            false, binding.accountId(), binding.generation());
    }

    void active(boolean value) {
        // 换输入框、重启输入和收起再弹出只作废这一轮生命周期的结果，不动接收策略的代次：那样回来的结果会先推进云端水位再被丢弃，这条远端复制就再也收不到。只有本机复制才永久放弃在途修订。
        active = value;
        lifecycleGeneration++;
        service.main.removeCallbacks(poll);
        if (active) service.main.post(poll);
    }

    void changed(MSIMEInputService.PrimaryClip clip) {
        ClipboardManager manager = service.getSystemService(ClipboardManager.class);
        android.content.ClipDescription description = manager == null ? null : manager.getPrimaryClipDescription();
        // 标记随剪贴板本身存活，换输入框、换号或服务重启后也不能把接收内容重新上传。
        if (ownWrite(description)) return;
        if (!policy.localChanged(clip == null ? null : clip.text())) return;
        if (clip != null && !clip.sensitive()) service.imePanels.autoSyncClipboardText(clip.text());
    }

    private static boolean ownWrite(android.content.ClipDescription description) {
        return description != null && LABEL.contentEquals(description.getLabel() == null ? "" : description.getLabel());
    }

    private void schedule(long delay) {
        service.main.removeCallbacks(poll);
        if (active) service.main.postDelayed(poll, delay);
    }

    private void poll() {
        if (!active) return;
        if (busy) { schedule(3_000); return; }
        if (!allowed()) { schedule(30_000); return; }
        String relay = relay();
        CloudClipboardReceivePolicy observer = policy;
        long generation = observer.generation();
        long lifecycle = lifecycleGeneration;
        busy = true;
        try {
            service.cloudClipboardWorker.execute(() -> {
                ImeCloudClipboardAutoSync.Binding expected = null;
                ImeCloudClipboardAutoSync.Binding after = null;
                BackendAccount.ClipboardPage page = null;
                try {
                    expected = resolve(service, relay);
                    ImeCloudClipboardAutoSync.Backend backend = ImeCloudClipboardAutoSync.backend(service);
                    if (expected != null && active && lifecycle == lifecycleGeneration && backend.current(expected)) {
                        page = ImeCloudClipboardAutoSync.page(service, expected);
                        if (!backend.current(expected)) page = null;
                    }
                    after = expected == null ? null : resolve(service, relay);
                } catch (Exception | LinkageError ignored) {
                    // 不保存失败内容，也不在输入过程中弹出网络错误。
                }
                ImeCloudClipboardAutoSync.Binding requested = expected;
                ImeCloudClipboardAutoSync.Binding settled = after;
                BackendAccount.ClipboardPage received = page;
                service.main.post(() -> {
                    busy = false;
                    // 请求期间换了输入框或重启了输入：结果作废，但不能交给 receive 推进水位，下一轮照常快速重查。
                    if (lifecycle != lifecycleGeneration) { schedule(3_000); return; }
                    if (requested == null || !Objects.equals(requested, settled)) { schedule(30_000); return; }
                    if (!Objects.equals(owner, requested)) {
                        // 换号或换中继后水位从头算；这一轮结果属于旧策略，立即按新策略重查。
                        owner = requested;
                        policy = new CloudClipboardReceivePolicy();
                        schedule(0);
                        return;
                    }
                    if (active && observer == policy && allowed() && relay.equals(relay())) {
                        apply(observer, generation, received);
                    }
                    schedule(received != null && received.enabled() && !received.pending() ? 3_000 : 30_000);
                });
            });
        } catch (RejectedExecutionException ignored) {
            busy = false;
            active(false);
        }
    }

    private void apply(CloudClipboardReceivePolicy observer, long generation, BackendAccount.ClipboardPage page) {
        if (page == null || !page.enabled() || page.pending() || page.items().isEmpty()) return;
        BackendAccount.ClipboardItem latest = page.items().get(0);
        try {
            ClipboardManager manager = service.getSystemService(ClipboardManager.class);
            if (manager == null) return;
            MSIMEInputService.PrimaryClip local = service.readPrimaryClip();
            boolean replaceable = local != null ? !local.sensitive() : !manager.hasPrimaryClip();
            // 键盘自己写入的内容按本机时钟记时间戳，不能拿来和服务器时间比；顺序只看云端水位。
            boolean own = local != null && ownWrite(manager.getPrimaryClipDescription());
            if (observer.receive(generation, latest.text(), latest.updatedAt(),
                    local == null ? null : local.text(), local == null ? 0 : local.copiedAtMs(), replaceable, own)) {
                manager.setPrimaryClip(ClipData.newPlainText(LABEL, latest.text()));
            }
        } catch (SecurityException ignored) {
            observer.invalidate();
        }
    }
}
