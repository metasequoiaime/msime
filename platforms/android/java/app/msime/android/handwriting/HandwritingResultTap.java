package app.msime.android;

import java.util.List;
import java.util.Objects;

/**
 * 包在手写识别器外面的一层：每次识别成功时先把结果交给原来的监听者（键盘服务据此显示候选），再交给 {@link Observer}（手写面板据此显示拼音、叠写时自动上屏首选）。其余调用原样转给被包的识别器。
 */
public final class HandwritingResultTap implements HandwritingRecognizer {
    /** 识别结果的旁听者；在识别器回调的线程上调用，自己负责切回主线程。 */
    public interface Observer {
        void onResult(long revision, List<String> candidates);
    }

    private final HandwritingRecognizer delegate;
    private final Observer observer;

    public HandwritingResultTap(HandwritingRecognizer delegate, Observer observer) {
        this.delegate = Objects.requireNonNull(delegate, "delegate");
        this.observer = Objects.requireNonNull(observer, "observer");
    }

    @Override public Availability availability() { return delegate.availability(); }

    @Override public void download(DownloadListener listener) { delegate.download(listener); }

    @Override public void setPreContext(String preContext) { delegate.setPreContext(preContext); }

    @Override public void recognize(Request request, RecognitionListener listener) {
        Objects.requireNonNull(listener, "listener");
        delegate.recognize(request, new RecognitionListener() {
            @Override public void onResult(long revision, List<String> candidates) {
                listener.onResult(revision, candidates);
                observer.onResult(revision, candidates);
            }

            @Override public void onFailure(long revision) {
                listener.onFailure(revision);
            }
        });
    }

    @Override public void cancelPending() { delegate.cancelPending(); }

    @Override public void close() { delegate.close(); }
}
