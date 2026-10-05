package app.msime.android;

import java.util.List;

/**
 * 共享背单词状态的只读视图。
 *
 * <p>The shared layer answers every action with the whole status, so this holds one snapshot and
 * nothing else: which books exist, which is selected, what today's counts are, and the queue the
 * session should deal. No scheduling arithmetic lives here — the intervals, the ease and what a
 * lapse costs all belong to `client-core::vocabulary::schedule`, and a second implementation on
 * this side would quietly disagree with the one every other host uses.
 *
 * <p>Android-free on purpose, like {@link TypingStatisticsModel}: `org.json` is a stub in the SDK
 * jar and throws on a host JVM, so the decoding lives in {@link VocabularyReviewDocument} and
 * everything the page reasons about can be exercised by `check-host.sh`.
 */
public final class VocabularyReviewModel {
    /** Read a non-negative JSON count without org.json's lossy numeric coercion. */
    public static int strictCount(Object value) {
        if (!(value instanceof Integer) && !(value instanceof Long))
            throw new IllegalArgumentException("Expected JSON review count");
        long count = ((Number) value).longValue();
        if (count < 0 || count > Integer.MAX_VALUE)
            throw new IllegalArgumentException("JSON review count is out of range");
        return (int) count;
    }

    /** Read a JSON string without org.json's lossy scalar coercion. */
    public static String strictString(Object value) {
        if (!(value instanceof String))
            throw new IllegalArgumentException("Expected JSON review string");
        return (String) value;
    }

    /** Read a JSON boolean without org.json's lossy scalar coercion. */
    public static boolean strictBoolean(Object value) {
        if (!(value instanceof Boolean))
            throw new IllegalArgumentException("Expected JSON review boolean");
        return (Boolean) value;
    }

    /** One word list the session can draw from. */
    public record Wordbook(String id, String name, int total, boolean builtin) {}

    /** One card, already resolved from the wordbook by the shared layer. */
    public record Card(String word, String phonetic, String meaning) {}

    private final List<Wordbook> wordbooks;
    private final String selected;
    private final int newPerDay;
    private final int sessionLimit;
    private final int due;
    private final int answeredToday;
    private final int introducing;
    private final int remaining;
    private final List<Card> queue;

    public VocabularyReviewModel(
        List<Wordbook> wordbooks,
        String selected,
        int newPerDay,
        int sessionLimit,
        int due,
        int answeredToday,
        int introducing,
        int remaining,
        List<Card> queue) {
        this.wordbooks = List.copyOf(wordbooks);
        this.selected = selected == null ? "" : selected;
        this.newPerDay = newPerDay;
        this.sessionLimit = sessionLimit;
        this.due = due;
        this.answeredToday = answeredToday;
        this.introducing = introducing;
        this.remaining = remaining;
        this.queue = List.copyOf(queue);
    }

    public List<Wordbook> wordbooks() { return wordbooks; }

    /** The selected wordbook id, empty when the user has not picked one. */
    public String selected() { return selected; }

    public int newPerDay() { return newPerDay; }

    public int sessionLimit() { return sessionLimit; }

    /** 今日待复习. */
    public int due() { return due; }

    /** 已完成. */
    public int answeredToday() { return answeredToday; }

    public int introducing() { return introducing; }

    public int remaining() { return remaining; }

    public List<Card> queue() { return queue; }

    /** The card to show, or {@code null} when today's session is finished. */
    public Card current() { return queue.isEmpty() ? null : queue.get(0); }

    /** The selected book, or {@code null} when none is selected or it has gone. */
    public Wordbook selectedWordbook() {
        for (Wordbook book : wordbooks) {
            if (book.id().equals(selected)) return book;
        }
        return null;
    }

    /**
     * Whether the page should offer the picker rather than a card.
     *
     * <p>Separate from "the queue is empty": with no book chosen the page has nothing to offer and
     * says so, while an empty queue with a book chosen means the day is done. Telling a user who
     * has never picked a book that they have finished would be a lie.
     */
    public boolean needsWordbook() { return selected.isEmpty() || selectedWordbook() == null; }

    /** What the progress row reads, as 今日待复习 N / 已完成 M. */
    public String progressSummary() {
        return "今日待复习 " + due + " / 已完成 " + answeredToday;
    }
}
