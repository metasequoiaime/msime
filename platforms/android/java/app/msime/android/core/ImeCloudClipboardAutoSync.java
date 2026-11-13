package app.msime.android;

import android.content.Context;
import java.util.concurrent.CancellationException;
import java.util.concurrent.Executor;
import java.util.concurrent.RejectedExecutionException;

/** 自动上传只使用复制时的绑定；后台排队和列表查询不能把旧文本带到新账号或新中继。 */
final class ImeCloudClipboardAutoSync {
    record Binding(boolean relay, String id, long generation) {}

    interface Backend {
        boolean current(Binding expected) throws Exception;
        Connection open(Binding expected) throws Exception;
    }

    interface Connection {
        boolean ready() throws Exception;
        void upload(String text) throws Exception;
    }

    private ImeCloudClipboardAutoSync() {}

    static void submit(Executor worker, Binding expected, String text, Backend backend) {
        try {
            worker.execute(() -> {
                try {
                    if (!backend.current(expected)) return;
                    Connection connection = backend.open(expected);
                    if (!connection.ready() || !backend.current(expected)) return;
                    connection.upload(text);
                } catch (Exception | LinkageError ignored) {
                    // 不留持久队列、不弹提示；下一次新的复制独立重试。
                }
            });
        } catch (RejectedExecutionException ignored) {
            // 服务销毁时工作线程已关闭，剪贴板回调不能因此使输入法崩溃。
        }
    }

    static Backend backend(Context context) {
        return new Backend() {
            @Override public boolean current(Binding expected) throws Exception {
                // 即使复制时走账号，也重新查询共享偏好；配置了中继之后绝不能继续走账号。
                CloudClipboardRelay.forPanel(context, expected.relay(), expected.id());
                if (expected.relay()) return true;
                SyncSwitch.Binding actual = SyncSignals.binding(context);
                return actual != null && CloudClipboardPanelPolicy.acceptsBinding(
                    expected.id(), expected.generation(), actual.accountId(), actual.generation());
            }

            @Override public Connection open(Binding expected) throws Exception {
                CloudClipboardRelay route = CloudClipboardRelay.forPanel(
                    context, expected.relay(), expected.id());
                if (route != null) {
                    return new Connection() {
                        @Override public boolean ready() throws Exception {
                            BackendAccount.ClipboardPage page = BackendAccount.clipboardPage(
                                route.drive(CloudClipboardE2e.Operation.list("")));
                            return page.enabled() && !page.pending();
                        }
                        @Override public void upload(String text) throws Exception {
                            route.drive(CloudClipboardE2e.Operation.add(text));
                        }
                    };
                }
                BackendAccount account = new BackendAccount(context);
                BackendAccount.SessionCredential session = account.currentSession();
                if (session.token().isEmpty() || !expected.id().equals(session.sessionId()))
                    throw new CancellationException("account session changed");
                CloudClipboardE2e.Local local = CloudClipboardNativeDriver.local(context);
                return new Connection() {
                    @Override public boolean ready() throws Exception {
                        BackendAccount.ClipboardPage page = account.clipboard("", expected.id(), local);
                        return page.enabled() && !page.pending();
                    }
                    @Override public void upload(String text) throws Exception {
                        account.addClipboard(text, expected.id(), local);
                    }
                };
            }
        };
    }
}
