import app.msime.android.VocabularyReviewModel;
import app.msime.android.VocabularyReviewModel.Card;
import app.msime.android.VocabularyReviewModel.Wordbook;
import java.util.List;

public final class VocabularyReviewModelSmoke {
    public static void main(String[] args) {
        List<Wordbook> books = List.of(
            new Wordbook("cet-4", "CET-4", 4500, true),
            new Wordbook("user-1", "我的词表", 12, false));
        List<Card> queue = List.of(
            new Card("ubiquitous", "/juːˈbɪkwɪtəs/", "adj. 无处不在的"),
            new Card("ephemeral", "", "adj. 短暂的"));
        VocabularyReviewModel model =
            new VocabularyReviewModel(books, "cet-4", 20, 200, 24, 6, 2, 4474, queue);

        check(model.progressSummary().equals("今日待复习 24 / 已完成 6"),
            "the progress row reads as the page states it");
        check(model.current() != null && model.current().word().equals("ubiquitous"),
            "the first queued card is the one to show");
        check(model.selectedWordbook() != null && model.selectedWordbook().builtin(),
            "the selected book is resolved from its id");
        check(!model.needsWordbook(), "a selected book that exists needs no picker");

        // 没选词书和今天复习完了是两回事：对一个从没选过词书的人说「已完成」是撒谎。
        VocabularyReviewModel unselected =
            new VocabularyReviewModel(books, "", 20, 200, 0, 0, 0, 0, List.of());
        check(unselected.needsWordbook(), "no selection asks for a book");
        check(unselected.current() == null, "an empty queue has no card");

        // A selected book the library no longer has is the same situation: the user deleted it.
        VocabularyReviewModel missing =
            new VocabularyReviewModel(books, "gone", 20, 200, 0, 0, 0, 0, List.of());
        check(missing.needsWordbook(), "a selection pointing at nothing asks for a book");
        check(missing.selectedWordbook() == null, "a missing book resolves to nothing");

        // A finished day keeps its book: the page says the session is done, not that none was picked.
        VocabularyReviewModel finished =
            new VocabularyReviewModel(books, "cet-4", 20, 200, 0, 30, 0, 4470, List.of());
        check(!finished.needsWordbook(), "a finished day still has its book");
        check(finished.current() == null, "a finished day has no card");
        check(finished.progressSummary().equals("今日待复习 0 / 已完成 30"),
            "a finished day still reports what it did");

        // The lists are copied in, so a caller that keeps its own reference cannot reach in later.
        check(model.wordbooks().size() == 2 && model.queue().size() == 2, "both lists survive");
        try {
            model.queue().add(new Card("x", "", "y"));
            check(false, "the queue is not writable from outside");
        } catch (UnsupportedOperationException expected) {
            check(true, "the queue is not writable from outside");
        }

        check(VocabularyReviewModel.strictCount(Integer.valueOf(24)) == 24,
            "JSON review counts accept integers");
        check(VocabularyReviewModel.strictCount(Long.valueOf(Integer.MAX_VALUE))
                == Integer.MAX_VALUE, "large in-range review counts retain precision");
        expectFailure(() -> VocabularyReviewModel.strictCount(Double.valueOf(24.5)));
        expectFailure(() -> VocabularyReviewModel.strictCount(Double.valueOf(24.0)));
        expectFailure(() -> VocabularyReviewModel.strictCount(Boolean.TRUE));
        expectFailure(() -> VocabularyReviewModel.strictCount(Long.valueOf(-1)));
        expectFailure(() -> VocabularyReviewModel.strictCount(
            Long.valueOf((long) Integer.MAX_VALUE + 1)));

        System.out.println("VocabularyReviewModelSmoke passed");
    }

    private static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
        System.out.println("  ok  " + what);
    }

    private static void expectFailure(Runnable action) {
        try { action.run(); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("Malformed review count accepted");
    }
}
