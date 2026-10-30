import app.msime.android.EditorReadCache;

public final class EditorReadCacheSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    private static int reads;

    private static int countedCursor(int value) {
        reads++;
        return value;
    }

    private static CharSequence counted(CharSequence value) {
        reads++;
        return value;
    }

    public static void main(String[] args) {
        EditorReadCache cache = new EditorReadCache();
        Object editor = new Object();

        // 范围外每次都直接读，不留任何值。
        check(cache.cursor(editor, () -> countedCursor(3)) == 3);
        check(cache.cursor(editor, () -> countedCursor(4)) == 4);
        check(reads == 2);

        // 范围里每样只读一次；光标前、光标后和光标位置各自缓存。
        reads = 0;
        cache.open();
        check(cache.cursor(editor, () -> countedCursor(7)) == 7);
        check(cache.cursor(editor, () -> countedCursor(8)) == 7);
        check("hello".contentEquals(cache.before(editor, () -> counted("hello"))));
        check("hello".contentEquals(cache.before(editor, () -> counted("other"))));
        check(cache.after(editor, () -> counted(null)) == null);
        // 读失败得到的 null 也算读过，和直接读的结果一致，不会在同一范围里反复重试。
        check(cache.after(editor, () -> counted("x")) == null);
        check(reads == 3);

        // 输入法自己写了编辑器：之后的读取重新去读。
        cache.invalidate();
        check(cache.cursor(editor, () -> countedCursor(8)) == 8);
        check("hello!".contentEquals(cache.before(editor, () -> counted("hello!"))));
        check(reads == 5);

        // 换了输入连接：旧连接上读到的值一律不用。
        Object other = new Object();
        check(cache.cursor(other, () -> countedCursor(1)) == 1);
        check("new".contentEquals(cache.before(other, () -> counted("new"))));
        check(reads == 7);

        // 没有连接时不缓存。
        check(cache.cursor(null, () -> countedCursor(-1)) == -1);
        check(cache.cursor(null, () -> countedCursor(-1)) == -1);
        check(reads == 9);

        // 嵌套：内层关上时外层的缓存还在，最外层关上后丢掉，下一条消息里重新读。
        cache.open();
        check(cache.cursor(other, () -> countedCursor(2)) == 1);
        cache.close();
        check(cache.cursor(other, () -> countedCursor(2)) == 1);
        check(reads == 9);
        cache.close();
        check(cache.cursor(other, () -> countedCursor(2)) == 2);
        check(reads == 10);
        cache.open();
        check(cache.cursor(other, () -> countedCursor(5)) == 5);
        cache.close();
        check(reads == 11);

        // 多关一次不会把计数弄成负数，之后的范围照常生效。
        cache.close();
        cache.open();
        check(cache.cursor(editor, () -> countedCursor(6)) == 6);
        check(cache.cursor(editor, () -> countedCursor(9)) == 6);
        cache.close();
        check(reads == 12);
        System.out.println("EditorReadCache smoke passed");
    }
}
