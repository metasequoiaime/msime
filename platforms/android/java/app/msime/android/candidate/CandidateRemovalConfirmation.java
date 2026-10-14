package app.msime.android;

/**
 * 删除词条的两步确认：第一次选中长按菜单里的「删除词条…」只记下这一个候选并提示再选一次，第二次选到同一个候选才真正删除。删除在键盘里不可撤销，这样才不至于选错一次就把词删掉。
 *
 * <p>确认不能弹 `AlertDialog`：输入法服务没有 Activity 的窗口令牌，对话框要么加不上窗口、让输入法进程崩掉，要么抢走编辑器的窗口焦点，两种情况键盘都会被收起（#5605、#5653）。两步都留在同一个长按菜单里，与 iOS 的嵌套子菜单、鸿蒙的「再次选择以确认」是同一个交互。
 *
 * <p>候选身份是 Engine 给的 session/generation/index（与 {@link CandidateManagementAction} 的其它操作相同）：换一次引擎候选、换一个词或离开输入框，上一步记下的候选就作废，用户重新走两步。这里只做判断和记忆，不依赖 Android 运行时，供 JVM 冒烟直接调用。
 */
public final class CandidateRemovalConfirmation {
    // generation 是 Engine 的非负计数器，Long.MIN_VALUE 用来表示「没有记下候选」，与 CandidateGlossPolicy.strictOr(..., Long.MIN_VALUE) 的约定一致。
    private long session;
    private long generation = Long.MIN_VALUE;
    private long index = Long.MIN_VALUE;

    /**
     * 用户在长按菜单里选中「删除词条…」，这次选的是不是刚刚记下的那一个候选。
     *
     * @return true 表示两次选的是同一个候选，删除可以执行，记下的候选随之清掉；false 表示这是第一次，记下它并等用户再选一次
     */
    public boolean confirms(long session, long generation, long index) {
        boolean same = armed() && this.session == session && this.generation == generation
            && this.index == index;
        if (!same) {
            this.session = session;
            this.generation = generation;
            this.index = index;
            return false;
        }
        clear();
        return true;
    }

    /** 记下了一个等确认的候选吗。 */
    public boolean armed() {
        return generation != Long.MIN_VALUE;
    }

    /** 引擎换了候选（{@code apply}）或输入会话结束：上一步记下的候选作废。 */
    public void clear() {
        session = 0;
        generation = Long.MIN_VALUE;
        index = Long.MIN_VALUE;
    }
}
