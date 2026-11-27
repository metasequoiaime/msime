package app.msime.android;

/** 一次在线请求从云候选走到 AI 候选的状态：先在在线 worker 上，交给主线程用过结果后再回到 worker，各步之间不互相等待（{@link MSIMEInputService} 的 fetchOnlineCandidates）。 */
final class OnlineCandidateRequest {
    final long session;
    final String document;
    final long epoch;
    final String signature;
    boolean cloudComplete;
    boolean aiComplete;
    String aiDocument;
    String aiResolveDocument;

    OnlineCandidateRequest(long session, String document, long epoch, String signature) {
        this.session = session;
        this.document = document;
        this.epoch = epoch;
        this.signature = signature;
        this.aiDocument = document;
    }
}
