package app.msime.android;

import android.content.ClipData;
import android.content.ClipboardManager;
import app.msime.android.clipboard.CloudClipboardReceivePolicy;
import java.util.Objects;
import java.util.concurrent.RejectedExecutionException;

/** 输入视图可见期间收取新文本；账号校验和网络请求仍由既有加密驱动负责。 */
final class ImeCloudClipboardAutoReceive {
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

    private ImeCloudClipboardAutoSync.Binding binding() {
        if (!service.imePanels.cloudClipboardAllowed()
                || service.clipboardMode == ClipboardModePolicy.Mode.VISITOR) return null;
        ImePanels.CloudOwner current = service.imePanels.cloudClipboardOwner();
        return current == null ? null : new ImeCloudClipboardAutoSync.Binding(
            current.relay(), current.id(), current.generation());
    }

    void active(boolean value) {
        active = value;
        lifecycleGeneration++;
        policy.invalidate();
        service.main.removeCallbacks(poll);
        if (active) service.main.post(poll);
    }

    void changed(MSIMEInputService.PrimaryClip clip) {
        ClipboardManager manager = service.getSystemService(ClipboardManager.class);
        android.content.ClipDescription description = manager == null ? null : manager.getPrimaryClipDescription();
        // 标记随剪贴板本身存活，换输入框、换号或服务重启后也不能把接收内容重新上传。
        if (description != null && "水杉云剪贴板".contentEquals(description.getLabel() == null
                ? "" : description.getLabel())) return;
        if (!policy.localChanged(clip == null ? null : clip.text())) return;
        if (clip != null && !clip.sensitive()) service.imePanels.autoSyncClipboardText(clip.text());
    }

    private void schedule(long delay) {
        service.main.removeCallbacks(poll);
        if (active) service.main.postDelayed(poll, delay);
    }

    private void poll() {
        if (!active) return;
        if (busy) { schedule(3_000); return; }
        ImeCloudClipboardAutoSync.Binding expected = binding();
        if (expected == null) { schedule(30_000); return; }
        if (!Objects.equals(owner, expected)) {
            owner = expected;
            policy = new CloudClipboardReceivePolicy();
        }
        CloudClipboardReceivePolicy observer = policy;
        long generation = observer.generation();
        long lifecycle = lifecycleGeneration;
        busy = true;
        try {
            service.cloudClipboardWorker.execute(() -> {
                BackendAccount.ClipboardPage page = null;
                try {
                    ImeCloudClipboardAutoSync.Backend backend = ImeCloudClipboardAutoSync.backend(service);
                    if (active && lifecycle == lifecycleGeneration && backend.current(expected)) {
                        page = ImeCloudClipboardAutoSync.page(service, expected);
                        if (!backend.current(expected)) page = null;
                    }
                } catch (Exception | LinkageError ignored) {
                    // 不保存失败内容，也不在输入过程中弹出网络错误。
                }
                BackendAccount.ClipboardPage received = page;
                service.main.post(() -> {
                    busy = false;
                    if (active && observer == policy && Objects.equals(expected, binding())) {
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
            if (observer.receive(generation, latest.text(), latest.updatedAt(),
                    local == null ? null : local.text(), local == null ? 0 : local.copiedAtMs(), replaceable)) {
                manager.setPrimaryClip(ClipData.newPlainText("水杉云剪贴板", latest.text()));
            }
        } catch (SecurityException ignored) {
            observer.invalidate();
        }
    }
}
