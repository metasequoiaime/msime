package app.msime.android;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/** 在会话线程上执行真实调度器，原生边界只提供确定性的评分回复。 */
public final class PauseRerankControllerSmoke {
    private record Scheduled(Runnable task, long delay) {}
    private static final class Queue implements PauseRerankController.Scheduler {
        final List<Scheduled> tasks = new ArrayList<>();
        @Override public void post(Runnable task, long delay) { tasks.add(new Scheduled(task, delay)); }
        @Override public void remove(Runnable task) { tasks.removeIf(item -> item.task() == task); }
        Scheduled next() {
            check(!tasks.isEmpty(), "应有下一次会话线程回调");
            Scheduled next = tasks.remove(0);
            next.task().run();
            return next;
        }
        void drain() {
            for (int count = 0; !tasks.isEmpty(); count++) {
                check(count < 100, "停顿回调必须在有限次回复内结束");
                next();
            }
        }
    }
    private static final class Host implements PauseRerankController.Bridge<String> {
        long session = 11;
        final Queue queue = new Queue();
        final ArrayDeque<PauseRerankController.Reply<String>> replies = new ArrayDeque<>();
        final List<String> calls = new ArrayList<>();
        final List<String> views = new ArrayList<>();
        int errors;
        boolean failPoll;
        final PauseRerankController<String> controller = new PauseRerankController<>(queue, this,
            () -> session, value -> { views.add(value); controllerSchedule(); }, () -> errors++);
        private void controllerSchedule() { controller.schedule(true); }
        @Override public PauseRerankController.Reply<String> start(long handle) {
            calls.add("start:" + handle);
            return replies.remove();
        }
        @Override public PauseRerankController.Reply<String> poll(long handle, long token, boolean accept) {
            calls.add("poll:" + handle + ":" + token + ":" + accept);
            if (failPoll) { failPoll = false; throw new IllegalStateException("synthetic unreadable reply"); }
            return replies.remove();
        }
        void reply(String status, long token, boolean moved, String view) {
            replies.add(new PauseRerankController.Reply<>(status, token, moved, view));
        }
    }
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    private static void unchangedTextStillPublishesIdentity() {
        Host host = new Host();
        host.reply("done", 0, false, "generation:2");
        host.controller.schedule(true);
        check(host.queue.next().delay() == 300, "停顿仍为300ms");
        check(host.views.equals(List.of("generation:2")), "文字不变也必须发布新的候选代次");
        check(host.queue.tasks.isEmpty(), "发布视图时的render不能再启动重排");
    }
    private static void continuationYieldsBetweenSlices() {
        Host host = new Host();
        host.reply("continue", 0, false, "generation:2");
        host.reply("done", 0, true, "generation:3");
        host.controller.schedule(true);
        host.queue.next();
        check(host.queue.next().delay() == 0, "continue须排队让出线程");
        check(host.calls.equals(List.of("start:11", "start:11")), "每片继续走start");
        check(host.views.equals(List.of("generation:2", "generation:3")), "每片更新候选身份");
        check(host.queue.tasks.isEmpty(), "完成后停止调度");
    }
    private static void pendingIsAcceptedAndReturned() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.reply("pending", 77, false, null);
        host.reply("done", 0, true, "generation:2");
        host.controller.schedule(true);
        host.queue.next();
        check(host.queue.next().delay() == 16, "pending须限速轮询");
        host.queue.next();
        check(host.calls.equals(List.of("start:11", "poll:11:77:true", "poll:11:77:true")), "同令牌轮询至归还");
        check(host.views.equals(List.of("generation:2")), "接受评分完成视图");
        check(host.queue.tasks.isEmpty(), "完成后无遗留回调");
    }
    private static void cancelKeepsDiscardingUntilDone() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.reply("pending", 77, false, null);
        host.reply("done", 0, true, "stale");
        host.controller.cancel();
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false", "poll:11:77:false")), "取消后必须持续accept=false到归还");
        check(host.views.isEmpty(), "取消评分不发布视图");
    }
    private static void newPauseCannotClaimDiscardedToken() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        // 先执行新停顿回调，旧评分此时仍未完成。
        int newPause = host.queue.tasks.get(0).delay() == 300 ? 0 : 1;
        host.queue.tasks.remove(newPause).task().run();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false")), "归还前不能重新start认领旧令牌");
        host.reply("done", 0, false, null);
        host.queue.next();
        host.reply("done", 0, true, "fresh");
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false", "poll:11:77:false", "start:11")), "归还后才启动新评分");
        check(host.views.equals(List.of("fresh")), "仅发布新停顿");
    }
    private static void staleCallbackCannotClearFreshTimer() {
        Host host = new Host();
        host.controller.schedule(true);
        Runnable stale = host.queue.tasks.get(0).task();
        host.controller.schedule(true);
        stale.run();
        host.controller.cancel();
        check(host.queue.tasks.isEmpty(), "迟到旧回调不能清掉新定时器引用");
        check(host.calls.isEmpty(), "过期回调不触碰原生会话");
    }
    private static void destroyedSessionIsNeverPolled() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.reply("pending", 77, false, null);
        host.controller.cancel();
        check(!host.queue.tasks.isEmpty(), "运行中的取消评分须保留归还回调");
        Runnable staleDiscard = host.queue.tasks.get(0).task();
        host.session = 22;
        host.controller.schedule(true);
        staleDiscard.run();
        host.reply("done", 0, true, "fresh");
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false", "start:22")), "销毁旧句柄由Rust收尾，迟到回调不poll旧句柄");
        check(host.views.equals(List.of("fresh")), "重建会话仍可评分");
    }
    private static void malformedPendingDoesNotLoop() {
        Host host = new Host();
        host.reply("pending", 0, false, null);
        host.controller.schedule(true);
        host.queue.next();
        check(host.errors == 1, "没有合法令牌的pending应报协议失败");
        check(host.queue.tasks.isEmpty(), "无令牌不能无休止启动");
    }
    private static void failedAcceptStillReturnsModel() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.failPoll = true;
        host.reply("pending", 77, false, null);
        host.reply("done", 0, true, "stale");
        host.queue.next();
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:true", "poll:11:77:false", "poll:11:77:false")),
            "接受失败后不能遗失仍在原生任务中的模型");
        check(host.errors == 1 && host.views.isEmpty(), "失败只报协议错误，不发布旧评分");
    }
    private static void unreadableDiscardKeepsPolling() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.failPoll = true;
        host.controller.cancel();
        host.reply("done", 0, true, "stale");
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false", "poll:11:77:false")),
            "取消回复不可读不等于已归还");
        check(host.views.isEmpty(), "取消回包的视图不能发布");
    }
    private static void repeatedCancelDoesNotDuplicatePolling() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.reply("pending", 77, false, null);
        host.controller.cancel();
        host.controller.cancel();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false")), "重复取消同令牌不能重复启动轮询");
        host.reply("pending", 77, false, null);
        host.queue.next();
        check(host.queue.tasks.size() == 1, "第二次pending仍只有一个归还回调");
        host.reply("done", 0, false, null);
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false", "poll:11:77:false", "poll:11:77:false")),
            "所有归还轮询仍属于原令牌");
    }
    private static void oldDiscardCannotClearNewJob() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.reply("pending", 77, false, null);
        host.controller.cancel();
        Runnable stale = host.queue.tasks.get(0).task();
        host.reply("done", 0, false, null);
        host.queue.drain();
        host.reply("pending", 88, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.reply("pending", 88, false, null);
        host.controller.cancel();
        stale.run();
        check(host.queue.tasks.size() == 1, "旧归还回调不能清掉新令牌的计时器");
        host.reply("done", 0, false, null);
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false", "poll:11:77:false",
            "start:11", "poll:11:88:false", "poll:11:88:false")), "旧令牌结束后只轮询新令牌");
    }
    private static void absentSessionStartsNothing() {
        Host host = new Host();
        host.session = 0;
        host.controller.schedule(true);
        host.controller.cancel();
        check(host.queue.tasks.isEmpty() && host.calls.isEmpty(), "零句柄不启动原生评分或计时器");
    }
    public static void main(String[] args) {
        Runnable[] tests = {PauseRerankControllerSmoke::unchangedTextStillPublishesIdentity,
            PauseRerankControllerSmoke::continuationYieldsBetweenSlices,
            PauseRerankControllerSmoke::pendingIsAcceptedAndReturned,
            PauseRerankControllerSmoke::cancelKeepsDiscardingUntilDone,
            PauseRerankControllerSmoke::newPauseCannotClaimDiscardedToken,
            PauseRerankControllerSmoke::staleCallbackCannotClearFreshTimer,
            PauseRerankControllerSmoke::destroyedSessionIsNeverPolled,
            PauseRerankControllerSmoke::malformedPendingDoesNotLoop,
            PauseRerankControllerSmoke::failedAcceptStillReturnsModel,
            PauseRerankControllerSmoke::unreadableDiscardKeepsPolling,
            PauseRerankControllerSmoke::repeatedCancelDoesNotDuplicatePolling,
            PauseRerankControllerSmoke::oldDiscardCannotClearNewJob,
            PauseRerankControllerSmoke::absentSessionStartsNothing};
        int failed = 0;
        for (int i = 0; i < tests.length; i++) {
            try { tests[i].run(); }
            catch (AssertionError | RuntimeException error) {
                failed++;
                System.err.println("case " + (i + 1) + ": " + error);
            }
        }
        check(failed == 0, "停顿重排行为失败 " + failed + "/" + tests.length);
        System.out.println("PauseRerankControllerSmoke: " + tests.length + " passed");
    }
}
