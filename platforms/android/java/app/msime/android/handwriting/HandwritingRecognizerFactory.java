package app.msime.android;

import android.content.Context;
import java.lang.reflect.InvocationTargetException;

/** Keeps optional model SDK classes outside the lightweight Android preview classpath. */
public final class HandwritingRecognizerFactory {
    private static final String IMPLEMENTATION = "app.msime.android.MlKitHandwritingRecognizer";

    private HandwritingRecognizerFactory() {}

    public static HandwritingRecognizer create(Context context) {
        try {
            Object value = Class.forName(IMPLEMENTATION).getConstructor(Context.class)
                .newInstance(context);
            if (value instanceof HandwritingRecognizer recognizer) return recognizer;
        } catch (ClassNotFoundException | NoSuchMethodException | InstantiationException
                 | IllegalAccessException | InvocationTargetException | SecurityException
                 | LinkageError ignored) {
            // The standalone preview intentionally has no ML Kit dependency.
        }
        return new UnavailableRecognizer();
    }

    private static final class UnavailableRecognizer implements HandwritingRecognizer {
        @Override public Availability availability() { return Availability.UNAVAILABLE; }
        @Override public void download(DownloadListener listener) { listener.onFailure(); }
        @Override public void recognize(Request request, RecognitionListener listener) {
            listener.onFailure(request.revision());
        }
        @Override public void cancelPending() {}
        @Override public void close() {}
    }
}
