package app.msime.android;

import java.time.DateTimeException;
import java.time.Instant;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.time.format.DateTimeFormatter;
import java.util.Locale;

/** Parses server timestamps without letting representable instants escape into UI code. */
public final class TimestampDisplayPolicy {
    private static final DateTimeFormatter LOCAL_DAY =
        DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm", Locale.ROOT);

    private TimestampDisplayPolicy() {}

    /** Formats an RFC 3339 timestamp in the device zone, or blank when it cannot be displayed. */
    public static String formatLocal(String value) {
        ZonedDateTime parsed = parse(value);
        return parsed == null ? "" : LOCAL_DAY.format(parsed);
    }

    /** Parses an instant or offset timestamp in the device zone, or null when it is unusable. */
    public static ZonedDateTime parse(String value) {
        if (value == null || value.isEmpty()) return null;
        try {
            return Instant.parse(value).atZone(ZoneId.systemDefault());
        } catch (DateTimeException ignored) {
            try {
                return ZonedDateTime.parse(value).withZoneSameInstant(ZoneId.systemDefault());
            } catch (DateTimeException unreadable) {
                return null;
            }
        }
    }
}
