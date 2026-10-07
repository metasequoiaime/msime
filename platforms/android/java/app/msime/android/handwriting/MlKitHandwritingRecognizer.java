package app.msime.android;

import android.content.Context;
import com.google.mlkit.common.MlKitException;
import com.google.mlkit.common.model.DownloadConditions;
import com.google.mlkit.common.model.RemoteModelManager;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognition;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognitionModel;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognitionModelIdentifier;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognizer;
import com.google.mlkit.vision.digitalink.recognition.DigitalInkRecognizerOptions;
import com.google.mlkit.vision.digitalink.recognition.Ink;
import com.google.mlkit.vision.digitalink.recognition.RecognitionContext;
import com.google.mlkit.vision.digitalink.recognition.WritingArea;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/** Google ML Kit adapter for the optional Chinese on-device handwriting model. */
public final class MlKitHandwritingRecognizer implements HandwritingRecognizer {
    private final Object lock = new Object();
    private final DigitalInkRecognitionModel model;
    private final DigitalInkRecognizer recognizer;
    private final RemoteModelManager modelManager;
    private volatile Availability availability = Availability.DOWNLOADING;
    private long generation;
    private boolean closed;
    private volatile String preContext = "";

    public MlKitHandwritingRecognizer(Context context) throws MlKitException {
        Objects.requireNonNull(context, "context");
        DigitalInkRecognitionModelIdentifier identifier =
            DigitalInkRecognitionModelIdentifier.fromLanguageTag("zh-Hani-CN");
        if (identifier == null) {
            throw new IllegalStateException("Chinese handwriting model unavailable");
        }
        model = DigitalInkRecognitionModel.builder(identifier).build();
        recognizer = DigitalInkRecognition.getClient(
            DigitalInkRecognizerOptions.builder(model).build());
        modelManager = RemoteModelManager.getInstance();
        long checkGeneration;
        synchronized (lock) { checkGeneration = generation; }
        modelManager.isModelDownloaded(model)
            .addOnSuccessListener(downloaded -> {
                synchronized (lock) {
                    if (closed || generation != checkGeneration) return;
                    availability = downloaded ? Availability.READY : Availability.DOWNLOAD_REQUIRED;
                }
            })
            .addOnFailureListener(ignored -> {
                synchronized (lock) {
                    if (closed || generation != checkGeneration) return;
                    availability = Availability.DOWNLOAD_REQUIRED;
                }
            });
    }

    @Override public Availability availability() { return availability; }

    @Override public void setPreContext(String value) {
        preContext = HandwritingRecognizer.clipPreContext(value);
    }

    @Override public void download(DownloadListener listener) {
        Objects.requireNonNull(listener, "listener");
        final long token;
        synchronized (lock) {
            if (closed) {
                listener.onFailure();
                return;
            }
            if (availability == Availability.READY) {
                listener.onProgress(100);
                listener.onComplete();
                return;
            }
            token = ++generation;
            availability = Availability.DOWNLOADING;
        }
        listener.onProgress(0);
        modelManager.download(model, new DownloadConditions.Builder().build())
            .addOnSuccessListener(ignored -> {
                synchronized (lock) {
                    if (closed || generation != token) return;
                    availability = Availability.READY;
                    listener.onProgress(100);
                    listener.onComplete();
                }
            })
            .addOnFailureListener(ignored -> {
                synchronized (lock) {
                    if (closed || generation != token) return;
                    availability = Availability.DOWNLOAD_REQUIRED;
                    listener.onFailure();
                }
            });
    }

    @Override public void recognize(Request request, RecognitionListener listener) {
        Objects.requireNonNull(request, "request");
        Objects.requireNonNull(listener, "listener");
        final long token;
        synchronized (lock) {
            if (closed || availability != Availability.READY) {
                listener.onFailure(request.revision());
                return;
            }
            token = ++generation;
        }
        Ink.Builder ink = Ink.builder();
        for (List<HandwritingInk.Point> points : request.strokes()) {
            Ink.Stroke.Builder stroke = Ink.Stroke.builder();
            for (HandwritingInk.Point point : points) {
                stroke.addPoint(Ink.Point.create(point.x(), point.y(), point.timeMillis()));
            }
            ink.addStroke(stroke.build());
        }
        RecognitionContext context = RecognitionContext.builder()
            .setPreContext(preContext)
            .setWritingArea(new WritingArea(request.width(), request.height()))
            .build();
        recognizer.recognize(ink.build(), context)
            .addOnSuccessListener(result -> {
                List<String> candidates = new ArrayList<>(result.getCandidates().size());
                result.getCandidates().forEach(candidate -> candidates.add(candidate.getText()));
                synchronized (lock) {
                    if (closed || generation != token) return;
                    listener.onResult(request.revision(),
                        HandwritingRecognizer.sanitizeCandidates(candidates));
                }
            })
            .addOnFailureListener(ignored -> {
                synchronized (lock) {
                    if (closed || generation != token) return;
                    listener.onFailure(request.revision());
                }
            });
    }

    @Override public void cancelPending() {
        synchronized (lock) { generation++; }
    }

    @Override public void close() {
        synchronized (lock) {
            if (closed) return;
            closed = true;
            generation++;
            availability = Availability.UNAVAILABLE;
        }
        recognizer.close();
    }
}
