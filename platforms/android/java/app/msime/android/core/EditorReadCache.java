package app.msime.android;

import java.util.function.IntSupplier;
import java.util.function.Supplier;

/**
 * 英文直输打一个字母，候选条、清除键、自动大写和渲染各自去读光标位置和光标前后文，同一份内容要读二十来次；每一次都是阻塞输入法主线程、等应用 UI 线程回答的跨进程调用，低端机上应用正为刚上屏的字重排版时还要等得更久。这里让一段没有写入编辑器的代码里每样只读一次。不依赖 Android，便于在 JVM 上测试。
 *
 * <p>只在调用方用 {@link #open()} / {@link #close()} 显式框起来的范围里缓存，范围外照常每次直接读。范围可以嵌套，最外层关上时丢掉缓存，所以读到的值不会留到下一条主线程消息里，应用自己改了文字也不会被旧值挡住。范围里输入法自己写了编辑器（上屏、删除、发按键）要调 {@link #invalidate()}；换了输入连接时自动重新读。读失败得到的 null 也照样缓存，和直接读的结果一致。
 */
public final class EditorReadCache {
    private int depth;
    private Object target;
    private boolean cursorRead;
    private int cursor;
    private boolean beforeRead;
    private CharSequence before;
    private boolean afterRead;
    private CharSequence after;

    public void open() {
        depth++;
    }

    public void close() {
        if (depth > 0 && --depth == 0) invalidate();
    }

    /** 输入法刚写过编辑器，之前读到的都可能过时。 */
    public void invalidate() {
        target = null;
        cursorRead = false;
        cursor = 0;
        beforeRead = false;
        before = null;
        afterRead = false;
        after = null;
    }

    /** 绝对光标位置（读不到时调用方自己约定的值，例如 -1）。 */
    public int cursor(Object connection, IntSupplier read) {
        if (!caching(connection)) return read.getAsInt();
        if (!cursorRead) {
            cursor = read.getAsInt();
            cursorRead = true;
        }
        return cursor;
    }

    /** 光标前一段固定长度的文字；同一范围里所有调用方必须读同样的长度。 */
    public CharSequence before(Object connection, Supplier<CharSequence> read) {
        if (!caching(connection)) return read.get();
        if (!beforeRead) {
            before = read.get();
            beforeRead = true;
        }
        return before;
    }

    /** 光标后一段固定长度的文字；同一范围里所有调用方必须读同样的长度。 */
    public CharSequence after(Object connection, Supplier<CharSequence> read) {
        if (!caching(connection)) return read.get();
        if (!afterRead) {
            after = read.get();
            afterRead = true;
        }
        return after;
    }

    private boolean caching(Object connection) {
        if (depth == 0 || connection == null) return false;
        if (connection != target) {
            invalidate();
            target = connection;
        }
        return true;
    }
}
