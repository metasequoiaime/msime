package app.msime.android;

import android.text.Spanned;
import android.view.inputmethod.ExtractedTextRequest;
import android.view.inputmethod.InputConnection;

public final class ChineseOutputCompositionSync {
    private ChineseOutputCompositionSync() {}

    public static void refresh(InputConnection connection, EditorBridge bridge,
            EditorBridge.Sink sink, String editing) {
        if (connection == null) return;
        try {
            ExtractedTextRequest request = new ExtractedTextRequest();
            request.flags = InputConnection.GET_TEXT_WITH_STYLES;
            var actual = connection.getExtractedText(request, 0);
            if (actual == null || actual.partialStartOffset != -1) return;
            refreshMarked(bridge, sink, editing, actual.text, actual.selectionStart, actual.selectionEnd);
        } catch (RuntimeException disconnected) {
        }
    }

    public static String readMarked(InputConnection connection, int composingStart, int composingEnd) {
        if (connection == null || composingStart < 0 || composingEnd <= composingStart) return null;
        try {
            ExtractedTextRequest request = new ExtractedTextRequest();
            request.flags = InputConnection.GET_TEXT_WITH_STYLES;
            var actual = connection.getExtractedText(request, 0);
            if (actual == null || actual.startOffset < 0 || actual.partialStartOffset != -1) return null;
            return markedText(actual.text, composingStart - actual.startOffset,
                composingEnd - actual.startOffset);
        } catch (RuntimeException disconnected) {
            return null;
        }
    }

    public static String markedText(CharSequence text, int composingStart, int composingEnd) {
        if (!(text instanceof Spanned marked) || composingStart < 0
                || composingEnd <= composingStart || composingEnd > marked.length()) return null;
        int start = Integer.MAX_VALUE;
        int end = -1;
        for (Object span : marked.getSpans(0, marked.length(), Object.class)) {
            if ((marked.getSpanFlags(span) & Spanned.SPAN_COMPOSING) == 0) continue;
            start = Math.min(start, marked.getSpanStart(span));
            end = Math.max(end, marked.getSpanEnd(span));
        }
        if (start != composingStart || end != composingEnd) return null;
        return marked.subSequence(start, end).toString();
    }

    public static void refreshMarked(EditorBridge bridge, EditorBridge.Sink sink,
            String editing, CharSequence text, int selectionStart, int selectionEnd) {
        if (!(text instanceof Spanned marked)) return;
        int start = Integer.MAX_VALUE;
        int end = -1;
        for (Object span : marked.getSpans(0, marked.length(), Object.class)) {
            if ((marked.getSpanFlags(span) & Spanned.SPAN_COMPOSING) == 0) continue;
            start = Math.min(start, marked.getSpanStart(span));
            end = Math.max(end, marked.getSpanEnd(span));
        }
        if (start < 0 || end <= start || end > marked.length()) return;
        bridge.refreshComposition(sink, editing, marked.subSequence(start, end).toString(),
            start, end, selectionStart, selectionEnd);
    }
}
