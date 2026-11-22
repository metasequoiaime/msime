import app.msime.android.CandidateHoldGate;
import java.util.ArrayList;
import java.util.List;

/** 按着候选时异步结果等到松手：用假的主线程队列驱动。 */
public final class CandidateHoldGateSmoke {
    private record Scheduled(Runnable task, long delay) {}

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
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
        System.out.println("CandidateHoldGateSmoke: passed");
    }
}
