import app.msime.android.CandidateHoldGate;
import java.util.ArrayList;
import java.util.List;

/** 按着候选时异步结果等到松手：用假的主线程队列驱动。 */
public final class CandidateHoldGateSmoke {
    private record Scheduled(Runnable task, long delay) {}

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    /** 展开面板开着：云/AI 候选等 release()，期间不轮询；只等松手的任务不受面板影响。 */
    private static void panelHold() {
        List<Scheduled> queue = new ArrayList<>();
        boolean[] held = {false};
        boolean[] open = {true};
        boolean[] stale = {false};
        List<String> ran = new ArrayList<>();
        CandidateHoldGate gate = new CandidateHoldGate(
            (task, delay) -> queue.add(new Scheduled(task, delay)), () -> held[0], () -> open[0]);

        gate.runWhenFree(() -> ran.add("resolve"), () -> false);
        check(ran.equals(List.of("resolve")), "只等松手的任务面板开着也照常执行");

        gate.runWhenPanelFree(2, () -> ran.add("cloud"), () -> false);
        check(ran.size() == 1 && gate.waitingTags().equals(List.of(2)), "面板开着时云候选等着，提示看得到");
        check(queue.size() == 1 && queue.get(0).delay() == 16, "排进来时先排一次复查");
        gate.release();
        check(queue.size() == 2 && queue.get(1).delay() == 0, "复查还排着时放行，也马上复查");
        queue.remove(1).task().run();
        check(ran.equals(List.of("resolve", "cloud")), "放行马上用上，不等一帧");
        queue.remove(0).task().run();
        check(ran.size() == 2 && queue.isEmpty(), "旧的那次复查到时什么也不做");
        ran.remove(1);
        gate.runWhenPanelFree(2, () -> ran.add("cloud"), () -> false);
        queue.remove(0).task().run();
        check(ran.size() == 1 && queue.isEmpty(), "面板开着时不轮询");
        gate.runWhenPanelFree(3, () -> ran.add("ai"), () -> false);
        gate.runWhenFree(() -> ran.add("after"), () -> false);
        check(gate.waitingTags().equals(List.of(2, 3)), "按到达的先后报来源");
        while (!queue.isEmpty()) queue.remove(0).task().run();
        check(ran.size() == 1, "排在面板后面的不插队");

        gate.release();
        check(queue.size() == 1 && queue.get(0).delay() == 0, "放行排一次马上复查");
        queue.remove(0).task().run();
        check(ran.equals(List.of("resolve", "cloud", "ai", "after")) && gate.waitingTags().isEmpty(),
            "刷新：面板还开着，在等的按先后用上");

        ran.clear();
        gate.runWhenPanelFree(2, () -> ran.add("later"), () -> false);
        check(ran.isEmpty() && gate.waitingTags().equals(List.of(2)), "放行之后再到的照常等");
        held[0] = true;
        gate.release();
        queue.remove(0).task().run();
        check(ran.isEmpty() && queue.size() == 1 && queue.get(0).delay() == 0, "旧的复查作废，只剩马上的那次");
        queue.remove(0).task().run();
        check(ran.isEmpty() && queue.size() == 1 && queue.get(0).delay() == 16, "放行了但按着候选：隔一帧再看");
        held[0] = false;
        open[0] = false;
        queue.remove(0).task().run();
        check(ran.equals(List.of("later")), "松手后执行");

        ran.clear();
        open[0] = true;
        gate.runWhenPanelFree(2, () -> ran.add("old"), () -> stale[0]);
        stale[0] = true;
        check(gate.waitingTags().isEmpty(), "放弃了的不再提示");
        open[0] = false;
        gate.release();
        while (!queue.isEmpty()) queue.remove(0).task().run();
        check(ran.isEmpty(), "放弃了的不执行");

        gate.runWhenPanelFree(2, () -> ran.add("closed"), () -> false);
        check(ran.equals(List.of("closed")), "面板收着时当场执行");
    }

    public static void main(String[] args) {
        List<Scheduled> queue = new ArrayList<>();
        boolean[] held = {false};
        boolean[] abandoned = {false};
        List<String> ran = new ArrayList<>();
        CandidateHoldGate gate = new CandidateHoldGate(
            (task, delay) -> queue.add(new Scheduled(task, delay)), () -> held[0]);

        gate.runWhenFree(() -> ran.add("free"), () -> false);
        check(ran.equals(List.of("free")) && queue.isEmpty(), "没按着时当场执行");

        held[0] = true;
        gate.runWhenFree(() -> ran.add("first"), () -> false);
        gate.runWhenFree(() -> ran.add("second"), () -> false);
        check(ran.size() == 1 && queue.size() == 1, "按着时排队等待，只排一个复查");
        check(queue.get(0).delay() == 16, "隔一帧再看");
        queue.remove(0).task().run();
        check(ran.size() == 1 && queue.size() == 1, "还按着就继续等");

        held[0] = false;
        while (!queue.isEmpty()) queue.remove(0).task().run();
        check(ran.equals(List.of("free", "first", "second")), "松手后按排队的先后执行");

        ran.clear();
        held[0] = true;
        gate.runWhenFree(() -> ran.add("queued"), () -> false);
        held[0] = false;
        gate.runWhenFree(() -> ran.add("behind"), () -> false);
        check(ran.isEmpty(), "前面还有在等的，后来的不插队");
        while (!queue.isEmpty()) queue.remove(0).task().run();
        check(ran.equals(List.of("queued", "behind")), "复查时依次执行");

        ran.clear();
        held[0] = true;
        gate.runWhenFree(() -> ran.add("late"), () -> abandoned[0]);
        abandoned[0] = true;
        held[0] = false;
        while (!queue.isEmpty()) queue.remove(0).task().run();
        check(ran.isEmpty(), "等的一方放弃之后不再执行");

        gate.runWhenFree(() -> ran.add("never"), () -> true);
        check(ran.isEmpty() && queue.isEmpty(), "已经放弃的不执行也不排队");
        panelHold();
        System.out.println("CandidateHoldGateSmoke: passed");
    }
}
