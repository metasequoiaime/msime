package app.msime.android;

import android.content.Context;
import java.util.concurrent.Executor;
import java.util.concurrent.RejectedExecutionException;

/** 复制事件的上传任务始终绑定到复制时的目的地，不跟随排队期间的换号或换路。 */
final class CloudClipboardAutoSync {
    interface Destination {
        boolean ready() throws Exception;
        void upload(String text) throws Exception;
    }

    interface Destinations {
        Destination open(String binding) throws Exception;
    }

    private CloudClipboardAutoSync() {}

    static void enqueue(Executor worker, Context context, boolean relay, String binding, String text) {
        enqueue(worker, binding, text, expected -> {
            // 账号分支同样复核选路，刚切到中继时不能继续上传到账号。
            CloudClipboardRelay route = CloudClipboardRelay.forPanel(context, relay, expected);
            if (route != null) return new Destination() {
                public boolean ready() throws Exception {
                    BackendAccount.ClipboardPage page = BackendAccount.clipboardPage(
                        route.drive(CloudClipboardE2e.Operation.list("")));
                    return page.enabled() && !page.pending();
                }
                public void upload(String value) throws Exception {
                    route.drive(CloudClipboardE2e.Operation.add(value));
                }
            };
            BackendAccount account = new BackendAccount(context);
            CloudClipboardE2e.Local local = CloudClipboardNativeDriver.local(context);
            return new Destination() {
                public boolean ready() throws Exception {
                    BackendAccount.ClipboardPage page = account.clipboard("", expected, local);
                    return page.enabled() && !page.pending();
                }
                public void upload(String value) throws Exception {
                    CloudClipboardRelay.forPanel(context, false, expected);
                    account.addClipboard(value, expected, local);
                }
            };
        });
    }

    static void enqueue(Executor worker, String binding, String text, Destinations destinations) {
        try {
            worker.execute(() -> {
                try {
                    Destination destination = destinations.open(binding);
                    if (destination.ready()) destination.upload(text);
                } catch (Exception | LinkageError ignored) {
                    // 下一次复制重新读取状态；不重试、不落盘，也不打断输入。
                }
            });
        } catch (RejectedExecutionException ignored) {
            // 输入法服务已销毁，不能让关闭的工作线程使剪贴板监听崩溃。
        }
    }
}
