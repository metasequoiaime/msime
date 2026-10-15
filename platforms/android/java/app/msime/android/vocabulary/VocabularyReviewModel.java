package app.msime.android;

import java.util.List;

/**
 * 共享背单词状态的只读视图。
 *
 * <p>共享层对每个操作都回传完整状态，所以这里只保存一份快照，别的什么都不存：有哪些词书、选中了哪本、今天的计数是多少，以及本轮该发的队列。这里不做任何排期计算——间隔、难度系数和遗忘的代价全部归 `client-services::vocabulary::schedule` 管，在这一侧再实现一遍只会悄悄地和其他所有宿主用的那份不一致。
 *
 * <p>和 {@link TypingStatisticsModel} 一样有意不依赖 Android：`org.json` 在 SDK jar 里只是桩，在主机 JVM 上会抛异常，所以解码放在 {@link VocabularyReviewDocument}，页面要推理的一切都能由 `check-host.sh` 跑到。
 */
public final class VocabularyReviewModel {
    /** Read a non-negative JSON count without org.json's lossy numeric coercion. */
    public static int strictCount(Object value) {
        Long raw = JsonPolicy.strictLong(value);
        if (raw == null) throw new IllegalArgumentException("Expected JSON review count");
        if (raw < 0 || raw > Integer.MAX_VALUE)
            throw new IllegalArgumentException("JSON review count is out of range");
        return raw.intValue();
    }

    /** Read a JSON string without org.json's lossy scalar coercion. */
    public static String strictString(Object value) {
        String parsed = JsonPolicy.strictString(value);
        if (parsed == null)
            throw new IllegalArgumentException("Expected JSON review string");
        return parsed;
    }

    /** Read a JSON boolean without org.json's lossy scalar coercion. */
    public static boolean strictBoolean(Object value) {
        Boolean parsed = JsonPolicy.strictBoolean(value);
        if (parsed == null)
            throw new IllegalArgumentException("Expected JSON review boolean");
        return parsed;
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
        this.wordbooks = ListPolicy.copyOrEmpty(wordbooks);
        this.selected = TextPolicy.emptyIfNull(selected);
        this.newPerDay = newPerDay;
        this.sessionLimit = sessionLimit;
        this.due = due;
        this.answeredToday = answeredToday;
        this.introducing = introducing;
        this.remaining = remaining;
        this.queue = ListPolicy.copyOrEmpty(queue);
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
