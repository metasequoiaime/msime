package app.msime.android;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * 在退格交给编辑器、还没确认的这几十毫秒里按住新开始的触摸，确认后按原顺序放出来（{@link BackspaceSettle}）。不依赖 Android，事件类型由调用方给出，便于在 JVM 上测试。
 *
 * <p>按住以整次触摸为单位：某次触摸的按下（`ACTION_DOWN`）落在按住期间，就把它和之后同一次触摸的全部事件（移动、第二根手指、抬起、取消）排进队列，放行时一起交出；放行时这次触摸还没结束，剩下的事件照常直接分发，顺序不乱。按住开始之前已经按下的触摸（比如按着的退格自己的左右滑）不受影响，其中途按下的第二根手指也照常分发：同一个事件流里没法只扣住一根手指。队列里已经有事件时，新的触摸也排在后面，不插队。
 *
 * <p>放行由调用方在确认或超时时调用 {@link #drain()}。调用方每次 {@link #offer} 之前先看 {@link #overflowing()}，队列过长（按住异常地久）就先放行，键盘不会因此吞掉输入，也不会打乱顺序。
 */
public final class TouchHoldQueue<E> {
    /** 和 `MotionEvent` 的动作常量相同。 */
    public static final int ACTION_DOWN = 0;
    public static final int ACTION_UP = 1;
    public static final int ACTION_CANCEL = 3;

    /** 排队事件的上限：一次正常的按住（≤ 100 ms）远到不了这么多。 */
    static final int MAX_QUEUED = 256;

    public interface Events<E> {
        /** 去掉指针下标后的动作（`getActionMasked()`）。 */
        int action(E event);
        /** 排队需要的副本：框架分发完会回收原事件。 */
        E copy(E event);
    }

    private final Events<E> events;
    private final ArrayDeque<E> queued = new ArrayDeque<>();
    /** 正在排队的这次触摸还没抬起或取消。 */
    private boolean queuingGesture;

    public TouchHoldQueue(Events<E> events) {
        this.events = events;
    }

    /**
     * 一个触摸事件到达。返回 true 表示已经排进队列，调用方这次不要分发；返回 false 就照常分发。
     *
     * <p>`holding` 是此刻是否在按住（有退格未确认）。
     */
    public boolean offer(E event, boolean holding) {
        int action = events.action(event);
        if (queuingGesture) {
            queued.add(events.copy(event));
            if (action == ACTION_UP || action == ACTION_CANCEL) queuingGesture = false;
            return true;
        }
        if (action != ACTION_DOWN || !holding && queued.isEmpty()) return false;
        queued.add(events.copy(event));
        queuingGesture = true;
        return true;
    }

    /** 队列过长：调用方先 {@link #drain()} 放行，再交新的事件。 */
    public boolean overflowing() {
        return queued.size() >= MAX_QUEUED;
    }

    /** 下一个排着的事件是一次新触摸的按下：放行到这里时如果又在按住（放出的退格又交给了编辑器），就停在这里，不让后面的触摸抢在那次退格前面。 */
    public boolean nextStartsGesture() {
        E next = queued.peek();
        return next != null && events.action(next) == ACTION_DOWN;
    }

    /** 逐个放行：交出最早排着的事件，没有时返回 null。队列放空时，还没结束的那次触摸之后的事件不再排队。 */
    public E poll() {
        E next = queued.poll();
        if (queued.isEmpty()) queuingGesture = false;
        return next;
    }

    /** 放行：按到达顺序交出排着的事件（副本，由调用方分发后回收）。还没结束的那次触摸，之后的事件不再排队。 */
    public List<E> drain() {
        List<E> released = new ArrayList<>(queued);
        queued.clear();
        queuingGesture = false;
        return released;
    }

    public boolean isEmpty() {
        return queued.isEmpty();
    }
}
