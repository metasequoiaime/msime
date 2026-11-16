package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executor;
import java.util.concurrent.RejectedExecutionException;

public final class ImeCloudClipboardAutoSyncSmoke {
    private static final ImeCloudClipboardAutoSync.Binding ACCOUNT =
        new ImeCloudClipboardAutoSync.Binding(false, "synthetic-account-a", 7);

    private static final class Worker implements Executor {
        private Runnable task;
        @Override public void execute(Runnable value) { task = value; }
        void run() { task.run(); }
    }

    private static final class Backend implements ImeCloudClipboardAutoSync.Backend {
        ImeCloudClipboardAutoSync.Binding binding = ACCOUNT;
        final List<String> uploads = new ArrayList<>();
        boolean ready = true;
        boolean fail;
        int opens;
        Runnable duringList = () -> {};
        @Override public boolean current(ImeCloudClipboardAutoSync.Binding expected) {
            return expected.equals(binding);
        }
        @Override public ImeCloudClipboardAutoSync.Connection open(ImeCloudClipboardAutoSync.Binding expected) {
            opens++;
            check(expected.equals(binding), "连接必须属于复制时的绑定");
            return new ImeCloudClipboardAutoSync.Connection() {
                @Override public boolean ready() throws Exception {
                    if (fail) throw new java.io.IOException("synthetic failure");
                    duringList.run();
                    return ready;
                }
                @Override public void upload(String text) { uploads.add(text); }
            };
        }
    }

    public static void main(String[] args) {
        Worker worker = new Worker();
        Backend backend = new Backend();
        ImeCloudClipboardAutoSync.submit(worker, ACCOUNT, "synthetic note", backend);
        check(backend.opens == 0, "复制回调不执行联网");
        worker.run();
        check(backend.uploads.equals(List.of("synthetic note")), "无需先打开面板即可上传");

        List<ImeCloudClipboardAutoSync.Binding> changed = new ArrayList<>();
        changed.add(null);
        changed.add(new ImeCloudClipboardAutoSync.Binding(false, "synthetic-account-b", 7));
        changed.add(new ImeCloudClipboardAutoSync.Binding(false, "synthetic-account-a", 8));
        changed.add(new ImeCloudClipboardAutoSync.Binding(true, "synthetic-relay", 0));
        for (ImeCloudClipboardAutoSync.Binding replacement : changed) {
            Backend queued = new Backend();
            ImeCloudClipboardAutoSync.submit(worker, ACCOUNT, "synthetic queued", queued);
            queued.binding = replacement;
            worker.run();
            check(queued.opens == 0 && queued.uploads.isEmpty(), "排队后换绑不得查询或上传");

            Backend inFlight = new Backend();
            inFlight.duringList = () -> inFlight.binding = replacement;
            ImeCloudClipboardAutoSync.submit(worker, ACCOUNT, "synthetic in-flight", inFlight);
            worker.run();
            check(inFlight.opens == 1 && inFlight.uploads.isEmpty(), "查询期间换绑不得上传");
        }
        Backend disabled = new Backend();
        disabled.ready = false;
        ImeCloudClipboardAutoSync.submit(worker, ACCOUNT, "synthetic disabled", disabled);
        worker.run();
        check(disabled.uploads.isEmpty(), "关闭或待批准的云剪贴板不得上传");

        Backend failed = new Backend();
        failed.fail = true;
        ImeCloudClipboardAutoSync.submit(worker, ACCOUNT, "synthetic failed", failed);
        worker.run();
        check(failed.uploads.isEmpty(), "失败不上传，也不打断复制回调");
        failed.fail = false;
        ImeCloudClipboardAutoSync.submit(worker, ACCOUNT, "synthetic next copy", failed);
        worker.run();
        check(failed.uploads.equals(List.of("synthetic next copy")), "后续复制可恢复，不重试旧文本");

        ImeCloudClipboardAutoSync.submit(task -> {
            throw new RejectedExecutionException("synthetic shutdown");
        }, ACCOUNT, "synthetic shutdown", new Backend());
        System.out.println("ImeCloudClipboardAutoSyncSmoke passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
