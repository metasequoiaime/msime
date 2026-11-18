import app.msime.android.TouchHoldQueue;
import java.util.List;

public final class TouchHoldQueueSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    /** 假事件：动作 + 序号，复制出新对象，便于确认交出的是副本、顺序不乱。 */
    record Event(int action, int serial) {}

    static final int DOWN = 0, UP = 1, MOVE = 2, CANCEL = 3, POINTER_DOWN = 5, POINTER_UP = 6;

    static TouchHoldQueue<Event> queue() {
        return new TouchHoldQueue<>(new TouchHoldQueue.Events<>() {
            @Override public int action(Event event) { return event.action(); }
            @Override public Event copy(Event event) { return new Event(event.action(), event.serial()); }
        });
    }

    static void serials(List<Event> events, int... expected) {
        check(events.size() == expected.length);
        for (int index = 0; index < expected.length; index++) check(events.get(index).serial() == expected[index]);
    }

    public static void main(String[] args) {
        // 不按住时一切照常分发。
        TouchHoldQueue<Event> idle = queue();
        check(!idle.offer(new Event(DOWN, 1), false));
        check(!idle.offer(new Event(MOVE, 2), false));
        check(!idle.offer(new Event(UP, 3), false));
        check(idle.isEmpty());

        // 退格刚交给编辑器，紧接着按下 n：整次触摸排队，确认后按原顺序放出，交出的是副本。
        TouchHoldQueue<Event> held = queue();
        Event down = new Event(DOWN, 1);
        check(held.offer(down, true));
        check(held.offer(new Event(MOVE, 2), true));
        check(held.offer(new Event(UP, 3), true));
        // 排着的触摸已经抬起，回声到达之前又按下一个键：同样排在后面。
        check(held.offer(new Event(DOWN, 4), true));
        List<Event> released = held.drain();
        serials(released, 1, 2, 3, 4);
        check(released.get(0) != down);
        // 放行时第二次触摸还没抬起：剩下的事件照常直接分发。
        check(!held.offer(new Event(MOVE, 5), false));
        check(!held.offer(new Event(UP, 6), false));
        check(held.isEmpty());

        // 回声已经到了、还没来得及放行（队列非空）时按下的新触摸不插队。
        TouchHoldQueue<Event> ordered = queue();
        check(ordered.offer(new Event(DOWN, 1), true));
        check(ordered.offer(new Event(UP, 2), true));
        check(ordered.offer(new Event(DOWN, 3), false));
        check(ordered.offer(new Event(UP, 4), false));
        serials(ordered.drain(), 1, 2, 3, 4);
        check(!ordered.offer(new Event(DOWN, 5), false));

        // 按住开始之前已经按下的触摸（按着的退格左右滑）不受影响，其中途按下的第二根手指也照常分发。
        TouchHoldQueue<Event> ongoing = queue();
        check(!ongoing.offer(new Event(DOWN, 1), false));
        check(!ongoing.offer(new Event(MOVE, 2), true));
        check(!ongoing.offer(new Event(POINTER_DOWN, 3), true));
        check(!ongoing.offer(new Event(POINTER_UP, 4), true));
        check(!ongoing.offer(new Event(UP, 5), true));
        check(ongoing.isEmpty());

        // 排着的触摸被取消：取消也排进去，之后的新触摸不按住就照常分发（要先放行）。
        TouchHoldQueue<Event> cancelled = queue();
        check(cancelled.offer(new Event(DOWN, 1), true));
        check(cancelled.offer(new Event(POINTER_DOWN, 2), true));
        check(cancelled.offer(new Event(CANCEL, 3), true));
        serials(cancelled.drain(), 1, 2, 3);
        check(!cancelled.offer(new Event(DOWN, 4), false));

        // 按住异常地久：队列满了，调用方先放行，顺序不乱。
        TouchHoldQueue<Event> full = queue();
        check(full.offer(new Event(DOWN, 0), true));
        int serial = 1;
        while (!full.overflowing()) check(full.offer(new Event(MOVE, serial++), true));
        List<Event> flushed = full.drain();
        check(flushed.size() == serial);
        check(flushed.get(flushed.size() - 1).serial() == serial - 1);
        check(!full.offer(new Event(MOVE, serial), true));

        System.out.println("TouchHoldQueue smoke passed");
    }
}
