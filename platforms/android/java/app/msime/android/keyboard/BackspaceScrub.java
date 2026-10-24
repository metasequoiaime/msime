package app.msime.android;

/** 一次未松手的退格按压；只持有有界纯文本，不接入持久撤回和剪贴板。 */
public final class BackspaceScrub {
    public static final int MAX_UNITS = 4096;
    public static final int CONTEXT_UNITS = 64;

    /** 一次读取中的绝对光标及有界前后文；未知位置和非折叠选区由宿主拒绝。 */
    public record Frame(int cursor, String before, String after) {}

    public interface Editor {
        Object target();
        Frame read();
        boolean delete(int units);
        boolean restore(String text);
    }

    private final Object target;
    private final int cursor;
    private final String prefix;
    private final String text;
    private final String after;
    private final boolean documentStart;
    private final int[] offsets;
    private int retained;
    private boolean valid = true;

    public BackspaceScrub(Object target, Frame frame) {
        this.target = target;
        cursor = frame.cursor();
        String before = frame.before();
        int start = Math.max(0, before.length() - MAX_UNITS);
        if (start < before.length() && Character.isLowSurrogate(before.charAt(start))) start++;
        text = before.substring(start);
        prefix = before.substring(Math.max(0, start - CONTEXT_UNITS), start);
        documentStart = cursor == text.length() + prefix.length();
        after = frame.after();
        offsets = new int[text.codePointCount(0, text.length()) + 1];
        for (int index = 1; index < offsets.length; index++)
            offsets[index] = offsets[index - 1] + Character.charCount(text.codePointAt(offsets[index - 1]));
        retained = offsets.length - 1;
    }

    public int count() { return offsets.length - 1; }
    public int deleted() { return count() - retained; }
    public boolean available() { return valid && count() > 0; }

    /** 普通按下和长按已经接受的码位删除也属于这次按压，可在水平滑回时恢复。 */
    public void ordinaryDelete(CharSequence preceding) {
        if (!valid) return;
        if (retained == 0 || preceding == null || preceding.length() == 0) {
            valid = false;
            return;
        }
        String last = text.substring(offsets[retained - 1], offsets[retained]);
        String actual = preceding.toString();
        if (!actual.endsWith(last)) valid = false;
        else retained--;
    }

    public void invalidate() { valid = false; }

    /** 写入前后都读回核实；false 可能只是异步投递失败，不能留着快照盲目重试。 */
    public boolean move(Editor editor, int deleted) {
        if (!valid || editor.target() != target) return fail();
        int next = count() - Math.max(0, Math.min(count(), deleted));
        if (next == retained) return true;
        if (!matches(editor.read())) return fail();
        boolean accepted = next < retained
            ? editor.delete(offsets[retained] - offsets[next])
            : editor.restore(text.substring(offsets[retained], offsets[next]));
        if (!accepted) return fail();
        retained = next;
        if (!matches(editor.read())) return fail();
        return true;
    }

    private boolean matches(Frame frame) {
        if (frame == null || frame.cursor() != cursor - text.length() + offsets[retained]
                || !after.equals(frame.after())) return false;
        String before = prefix + text.substring(0, offsets[retained]);
        return documentStart ? before.equals(frame.before()) : frame.before().endsWith(before);
    }

    private boolean fail() {
        valid = false;
        return false;
    }

    public static boolean starts(float dx, float dy, float density) {
        return dx >= 8f * density && dx > Math.abs(dy) * 1.5f;
    }

    /** 激活点保留已经发生的普通连删；回到按下点恢复全部，向左到边缘可删除有界前文。 */
    public static int deletionCount(float dx, float travel, int total, float activation, int initial) {
        if (dx <= 0 || total <= 0) return 0;
        if (dx < activation && activation > 0)
            return Math.min(total, Math.round(initial * dx / activation));
        float progress = Math.max(0f, Math.min(1f, (dx - activation) / Math.max(1f, travel - activation)));
        return Math.min(total, initial + Math.round((total - initial) * progress * progress));
    }
}
