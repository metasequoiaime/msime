package app.msime.android;

import java.util.function.LongSupplier;

/**
 * 取消掉的后台停顿评分（`msime_client_start_settled_rerank` 回 `pending` 的那种）仍要轮询到结束，模型才会还给会话；这里只丢弃，不接受也不发布候选。与鸿蒙键盘的 `PauseRerankDiscard` 相同。
 *
 * <p>不碰 JSON：轮询由调用方做（`pollSettledRerank(session, token, false)`），只交回回答里的 `status`，读不懂时交回 null。这样纯 Java 的冒烟里也能驱动它。
 */
final class PauseRerankDiscard {
    /** 轮询间隔，约一帧。 */
    static final long POLL_MILLIS = 16;

    /** 丢弃轮询一次，回答的 `status`；读不懂时 null。 */
    interface Poll {
        String status(long session, long token);
    }

    /** 主线程上的定时：返回一个能交给 {@link #cancel} 的句柄。 */
    interface Clock {
        Object schedule(Runnable task, long delayMillis);

        void cancel(Object handle);
    }

    private final Poll poll;
    private final LongSupplier currentSession;
    private final Clock clock;
    private long session;
    private long token;
    private Object timer;

    PauseRerankDiscard(Poll poll, LongSupplier currentSession, Clock clock) {
        this.poll = poll;
        this.currentSession = currentSession;
        this.clock = clock;
    }

    /** 这个会话还有没归还的旧任务：归还之前不能为这次停顿另起一个，令牌会被认领成这次的结果。 */
    boolean waiting(long session) {
        if (this.session != session) clear();
        return token != 0;
    }

    /** 开始丢弃 `token`：轮询到 `done` 或会话换掉为止。同一个令牌再交来什么都不做。 */
    void discard(long session, long token) {
        if (session == 0 || token == 0 || (this.session == session && this.token == token)) return;
        clear();
        this.session = session;
        this.token = token;
        poll(session, token);
    }

    private void poll(long session, long token) {
        // 旧定时器即使已在队列里，也不得清掉后来任务的定时器或碰已经换掉的会话。
        if (this.session != session || this.token != token) return;
        if (currentSession.getAsLong() != session) {
            clear();
            return;
        }
        timer = null;
        String status = null;
        try {
            status = poll.status(session, token);
        } catch (RuntimeException | LinkageError ignored) {
            // 读不懂回答不代表已经归还，继续丢弃到明确结束或会话销毁。
        }
        if ("done".equals(status)) {
            clear();
        } else {
            timer = clock.schedule(() -> poll(session, token), POLL_MILLIS);
        }
    }

    private void clear() {
        if (timer != null) clock.cancel(timer);
        timer = null;
        session = 0;
        token = 0;
    }
}
