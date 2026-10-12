package app.msime.android.home;

/** Shared compact text rules used by settings rows. */
final class HomeTextPolicy {
    private static final int SHORT_LABEL_LIMIT = 24;

    private HomeTextPolicy() {}

    /** Keeps row values to one line while retaining a visible truncation marker. */
    static String shortLabel(String value) {
        String single = value.replace('\n', ' ');
        return single.length() <= SHORT_LABEL_LIMIT ? single : single.substring(0, SHORT_LABEL_LIMIT - 1) + "…";
    }
}
