package app.msime.android;

/** 外部请求的录音、解码和一次性结果归属，取消和迟到回调共用同一终止闸门。 */
public final class OfflineVoiceSession {
    private enum State { READY, LISTENING, DECODING, FINISHED }
    private State state = State.READY;

    public synchronized boolean start() {
        if (state != State.READY) return false;
        state = State.LISTENING;
        return true;
    }

    public synchronized boolean stop() {
        if (state != State.LISTENING) return false;
        state = State.DECODING;
        return true;
    }

    public synchronized boolean finish() {
        if (state == State.FINISHED) return false;
        state = State.FINISHED;
        return true;
    }

    public synchronized boolean listening() {
        return state == State.LISTENING;
    }

    public synchronized boolean finished() {
        return state == State.FINISHED;
    }
}
