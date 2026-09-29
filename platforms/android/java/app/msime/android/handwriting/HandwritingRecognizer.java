package app.msime.android;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;

/** Async platform seam for an on-device handwriting model such as ML Kit Digital Ink. */
public interface HandwritingRecognizer extends AutoCloseable {
    int MAX_CANDIDATES = 12;
    int MAX_CANDIDATE_BYTES = 256;

    enum Availability { UNAVAILABLE, DOWNLOAD_REQUIRED, DOWNLOADING, READY }

    record Request(long revision, List<List<HandwritingInk.Point>> strokes,
                   float width, float height) {
        public Request {
            if (revision < 0 || strokes.isEmpty() || strokes.size() > HandwritingInk.MAX_STROKES
                    || !Float.isFinite(width) || !Float.isFinite(height)
                    || width <= 0 || height <= 0 || width > 4096 || height > 4096) {
                throw new IllegalArgumentException("Handwriting request is invalid");
            }
            List<List<HandwritingInk.Point>> copied = new ArrayList<>();
            for (List<HandwritingInk.Point> stroke : strokes) {
                if (stroke.isEmpty() || stroke.size() > HandwritingInk.MAX_POINTS_PER_STROKE) {
                    throw new IllegalArgumentException("Handwriting stroke is invalid");
                }
                if (stroke.stream().anyMatch(point -> point.x() > width || point.y() > height)) {
                    throw new IllegalArgumentException("Handwriting point is outside the writing area");
                }
                copied.add(List.copyOf(stroke));
            }
            strokes = List.copyOf(copied);
        }
    }

    interface DownloadListener {
        void onProgress(int percent);
        void onComplete();
        void onFailure();
    }

    interface RecognitionListener {
        void onResult(long revision, List<String> candidates);
        void onFailure(long revision);
    }

    Availability availability();
    void download(DownloadListener listener);
    void recognize(Request request, RecognitionListener listener);
    void cancelPending();
    @Override void close();

    static List<String> sanitizeCandidates(List<String> values) {
        if (values == null) return List.of();
        LinkedHashSet<String> accepted = new LinkedHashSet<>();
        for (String value : values) {
            if (value == null) continue;
            String candidate = value.strip();
            if (candidate.isEmpty()
                    || TextPolicy.utf8Length(candidate) > MAX_CANDIDATE_BYTES
                    || TextPolicy.hasControl(candidate)) continue;
            accepted.add(candidate);
            if (accepted.size() == MAX_CANDIDATES) break;
        }
        return List.copyOf(accepted);
    }
}
