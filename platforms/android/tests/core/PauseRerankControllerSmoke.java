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
        boolean held;
        final PauseRerankController<String> controller = new PauseRerankController<>(queue, this,
            () -> session, value -> { views.add(value); controllerSchedule(); }, () -> errors++, () -> held);
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
        void deferredReply(String status, String view) {
            replies.add(new PauseRerankController.Reply<>(status, 0, view != null, view, true));
        }
        /** 先跑完一次停顿，让控制器从回复里知道会话挂着延后的模型。 */
        void learnDeferred() {
            deferredReply("done", null);
            controller.schedule(true);
            queue.next();
            calls.clear();
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
    private static void heldCandidatePostponesThePause() {
        Host host = new Host();
        host.held = true;
        host.reply("done", 0, true, "generation:2");
        host.controller.schedule(true);
        host.queue.next();
        check(host.calls.isEmpty(), "按着候选时不起跑");
        check(host.queue.tasks.size() == 1 && host.queue.tasks.get(0).delay() == 16, "按着时隔一帧再看");
        host.queue.next();
        check(host.calls.isEmpty() && host.views.isEmpty(), "一直按着就一直不换代");
        host.held = false;
        host.queue.next();
        check(host.calls.equals(List.of("start:11")), "松手后接着起跑");
        check(host.views.equals(List.of("generation:2")), "松手后才发布");
    }
    private static void heldCandidateStopsBetweenSlices() {
        Host host = new Host();
        host.reply("continue", 0, false, "generation:2");
        host.reply("done", 0, true, "generation:3");
        host.controller.schedule(true);
        host.queue.next();
        host.held = true;
        host.queue.next();
        check(host.calls.equals(List.of("start:11")), "两片之间按下时不接续");
        check(host.views.equals(List.of("generation:2")), "按着时不发布下一片");
        host.held = false;
        host.queue.next();
        check(host.calls.equals(List.of("start:11", "start:11")), "松手后接续下一片");
        check(host.views.equals(List.of("generation:2", "generation:3")), "接续的一片照常发布");
    }
    private static void heldCandidateStillReturnsCancelledScore() {
        Host host = new Host();
        host.reply("pending", 77, false, null);
        host.controller.schedule(true);
        host.queue.next();
        host.held = true;
        host.reply("done", 0, false, null);
        host.controller.cancel();
        host.queue.drain();
        check(host.calls.equals(List.of("start:11", "poll:11:77:false")), "按着候选也照常归还取消的评分");
        check(host.views.isEmpty(), "归还不发布视图");
    }
    private static void deferredSessionStartsEarly() {
        Host host = new Host();
        host.learnDeferred();
        host.controller.schedule(true);
        check(host.queue.tasks.get(0).delay() == 40, "延后模型的会话停顿 40 ms 起跑");
        host.reply("done", 0, false, null);
        host.queue.next();
        host.controller.schedule(true);
        check(host.queue.tasks.get(0).delay() == 300, "回复说不再延后时回到 300 ms");
    }
    private static void deferredFlagBelongsToItsSession() {
        Host host = new Host();
        host.learnDeferred();
        host.session = 22;
        host.controller.schedule(true);
        check(host.queue.tasks.get(0).delay() == 300, "换了会话要等新会话的回复");
    }
    private static void unrelatedRedrawKeepsTheTimer() {
        Host host = new Host();
        host.controller.schedule(true);
        Runnable timer = host.queue.tasks.get(0).task();
        host.controller.keep(true);
        check(host.queue.tasks.size() == 1 && host.queue.tasks.get(0).task() == timer,
            "无关的重绘不重新计时");
        host.controller.keep(false);
        check(host.queue.tasks.isEmpty(), "组字不再适用时取消");
    }
    private static void redrawDoesNotStopRunningSlices() {
        Host host = new Host();
        host.reply("continue", 0, false, "generation:2");
        host.reply("done", 0, true, "generation:3");
        host.controller.schedule(true);
        host.queue.next();
        host.controller.keep(true);
        host.queue.drain();
        check(host.views.equals(List.of("generation:2", "generation:3")), "片与片之间的重绘不打断重解");
    }
    private static void settleNowPublishesTheLastSliceOnly() {
        Host host = new Host();
        host.learnDeferred();
        host.controller.schedule(true);
        host.deferredReply("continue", "generation:2");
        host.deferredReply("continue", "generation:3");
        host.deferredReply("continue", "generation:4");
        host.controller.settleNow();
        check(host.calls.equals(List.of("start:11", "start:11", "start:11")), "当场最多跑三片");
        check(host.views.equals(List.of("generation:4")), "只发布最后一片，中间的不画");
        check(host.queue.tasks.size() == 1 && host.queue.tasks.get(0).delay() == 0,
            "没解完的换成紧接着的下一片，原来的计时取消");
        host.deferredReply("done", "generation:5");
        host.queue.next();
        check(host.views.equals(List.of("generation:4", "generation:5")), "余下的照常接着做");
    }
    private static void settleNowStopsWhenDone() {
        Host host = new Host();
        host.learnDeferred();
        host.deferredReply("done", "generation:2");
        host.controller.settleNow();
        check(host.calls.equals(List.of("start:11")) && host.views.equals(List.of("generation:2")),
            "解完就停");
        check(host.queue.tasks.isEmpty(), "解完不再排");
    }
    private static void settleNowNeedsDeferredModel() {
        Host host = new Host();
        host.controller.schedule(true);
        host.controller.settleNow();
        check(host.calls.isEmpty(), "不知道挂着延后模型时不当场重解");
        check(host.queue.tasks.size() == 1 && host.queue.tasks.get(0).delay() == 300, "计时照旧");
    }
    private static void settleNowWaitsForHeldCandidate() {
        Host host = new Host();
        host.learnDeferred();
        host.controller.schedule(true);
        host.held = true;
        host.controller.settleNow();
        check(host.calls.isEmpty(), "按着候选时不当场重解");
        check(host.queue.tasks.size() == 1, "计时照旧");
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
            PauseRerankControllerSmoke::absentSessionStartsNothing,
            PauseRerankControllerSmoke::heldCandidatePostponesThePause,
            PauseRerankControllerSmoke::heldCandidateStopsBetweenSlices,
            PauseRerankControllerSmoke::heldCandidateStillReturnsCancelledScore,
            PauseRerankControllerSmoke::deferredSessionStartsEarly,
            PauseRerankControllerSmoke::deferredFlagBelongsToItsSession,
            PauseRerankControllerSmoke::unrelatedRedrawKeepsTheTimer,
            PauseRerankControllerSmoke::redrawDoesNotStopRunningSlices,
            PauseRerankControllerSmoke::settleNowPublishesTheLastSliceOnly,
            PauseRerankControllerSmoke::settleNowStopsWhenDone,
            PauseRerankControllerSmoke::settleNowNeedsDeferredModel,
            PauseRerankControllerSmoke::settleNowWaitsForHeldCandidate};
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
