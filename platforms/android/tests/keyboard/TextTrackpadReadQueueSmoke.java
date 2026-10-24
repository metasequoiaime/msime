import app.msime.android.TextTrackpadReadQueue;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executor;

/** 可控执行器验证实际合并队列，不靠定时睡眠制造竞态。 */
public final class TextTrackpadReadQueueSmoke {
    public static void main(String[] args) {
        Tasks worker = new Tasks();
        Tasks main = new Tasks();
        TextTrackpadReadQueue<String> queue = new TextTrackpadReadQueue<>(worker, main);
        List<String> read = new ArrayList<>();
        List<String> shown = new ArrayList<>();
        queue.request(() -> { read.add("first"); return "first"; }, shown::add);
        queue.request(() -> { read.add("superseded"); return "superseded"; }, shown::add);
        queue.request(() -> { read.add("latest"); return "latest"; }, shown::add);
        check(worker.size() == 1 && read.isEmpty(), "仅一个后台请求，主线程不读编辑器");
        worker.next();
        check(shown.isEmpty(), "后台不直接画 UI");
        main.next();
        check(shown.isEmpty() && worker.size() == 1, "迟到结果丢弃，只安排最新请求");
        worker.next();
        main.next();
        check(read.equals(List.of("first", "latest")) && shown.equals(List.of("latest")),
            "中间请求合并，只有最新结果显示");
        queue.request(() -> "old editor", shown::add);
        queue.cancel();
        queue.request(() -> "new editor", shown::add);
        worker.next();
        main.next();
        check(worker.size() == 1 && shown.size() == 1, "切换编辑器后旧结果丢弃");
        worker.next();
        main.next();
        check(shown.equals(List.of("latest", "new editor")), "新连接恢复读取");
        queue.request(() -> "hidden", shown::add);
        worker.next();
        queue.cancel();
        main.next();
        check(shown.size() == 2 && worker.size() == 0, "隐藏后已完成的回读也不显示");
        queue.request(() -> "closed", shown::add);
        queue.request(() -> "pending", shown::add);
        queue.close();
        worker.next();
        main.next();
        queue.request(() -> { throw new AssertionError("销毁后不能再次读取"); }, shown::add);
        check(shown.size() == 2 && worker.size() == 0, "销毁清除待处理请求并拒绝新请求");
        System.out.println("Android text trackpad read queue: coalescing, stale results and lifecycle passed");
    }

    private static final class Tasks implements Executor {
        private final ArrayDeque<Runnable> tasks = new ArrayDeque<>();
        @Override public void execute(Runnable task) { tasks.add(task); }
        int size() { return tasks.size(); }
        void next() {
            check(!tasks.isEmpty(), "存在预期回调");
            tasks.remove().run();
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
