import app.msime.android.VocabularyReviewModel.Wordbook;
import app.msime.android.VocabularyReviewText;
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;

public final class VocabularyReviewTextSmoke {
    public static void main(String[] args) {
        check(VocabularyReviewText.failureMessage("wordbook import contains no usable rows")
                .equals(VocabularyReviewText.UNREADABLE),
            "a file that is not a word list is reported as the file, not as storage");
        check(VocabularyReviewText.failureMessage("the vocabulary library is full")
                .equals(VocabularyReviewText.LIBRARY_FULL),
            "a full library asks for a removal first");
        check(VocabularyReviewText.failureMessage("vocabulary wordbook is not in the library")
                .equals(VocabularyReviewText.MISSING),
            "a book that has gone asks for a new choice");
        check(VocabularyReviewText.failureMessage("a bundled vocabulary wordbook cannot be deleted")
                .equals(VocabularyReviewText.BUILTIN),
            "a bundled book is not removable");
        check(VocabularyReviewText.failureMessage("vocabulary progress storage failed: denied")
                .equals(VocabularyReviewText.UNAVAILABLE),
            "anything else is a storage failure that keeps the progress");
        check(VocabularyReviewText.failureMessage(null).equals(VocabularyReviewText.UNAVAILABLE),
            "no error text is a storage failure too");
        check(VocabularyReviewText.MAX_IMPORT_BYTES == 8 * 1024 * 1024,
            "the import limit is the shared 8 MiB");

        check(VocabularyReviewText.wordbookLabel(new Wordbook("cet4", "CET-4", 3849, true))
                .equals("CET-4（3849 词）"),
            "a book is listed with its word count");

        check(VocabularyReviewText.importName("考研核心词.csv").equals("考研核心词"),
            "the extension is dropped from the imported name");
        check(VocabularyReviewText.importName("primary:Download/my.words.txt").equals("my.words"),
            "only the last extension and path segment are dropped");
        check(VocabularyReviewText.importName("  .csv").equals(".csv"),
            "a leading dot is a name, not an extension");
        check(VocabularyReviewText.importName("").equals("导入的词表"),
            "an unnamed file gets the default name");
        check(VocabularyReviewText.importName("   ").equals("导入的词表"),
            "a blank name gets the default name");
        String longName = "词".repeat(70) + ".txt";
        check(VocabularyReviewText.importName(longName).equals("词".repeat(64)),
            "a long name is cut to 64 characters");
        String emoji = "📚".repeat(70);
        String cut = VocabularyReviewText.importName(emoji);
        check(cut.codePointCount(0, cut.length()) == 64 && cut.length() == 128,
            "a long name is cut on code points, never inside a surrogate pair");

        String text = "ubiquitous,adj. 无处不在的\n";
        check(VocabularyReviewText.decodeImport(text.getBytes(StandardCharsets.UTF_8)).equals(text),
            "UTF-8 is read as UTF-8");
        byte[] bom = "\uFEFF".getBytes(StandardCharsets.UTF_8);
        byte[] withBom = new byte[bom.length + text.getBytes(StandardCharsets.UTF_8).length];
        System.arraycopy(bom, 0, withBom, 0, bom.length);
        System.arraycopy(text.getBytes(StandardCharsets.UTF_8), 0, withBom, bom.length, withBom.length - bom.length);
        check(VocabularyReviewText.decodeImport(withBom).equals("\uFEFF" + text),
            "a UTF-8 byte order mark is left for the shared parser");
        byte[] little = utf16(text, true);
        check(VocabularyReviewText.decodeImport(little).equals(text),
            "UTF-16 little-endian with a byte order mark is read as UTF-16");
        byte[] big = utf16(text, false);
        check(VocabularyReviewText.decodeImport(big).equals(text),
            "UTF-16 big-endian with a byte order mark is read as UTF-16");
        check(VocabularyReviewText.decodeImport(text.getBytes(Charset.forName("GB18030"))).equals(text),
            "a GB18030 file is read as GB18030");
        check(VocabularyReviewText.decodeImport(null).isEmpty(), "no bytes are no text");
        System.out.println("VocabularyReviewTextSmoke passed");
    }

    private static byte[] utf16(String text, boolean little) {
        byte[] body = text.getBytes(little ? StandardCharsets.UTF_16LE : StandardCharsets.UTF_16BE);
        byte[] out = new byte[body.length + 2];
        out[0] = (byte) (little ? 0xFF : 0xFE);
        out[1] = (byte) (little ? 0xFE : 0xFF);
        System.arraycopy(body, 0, out, 2, body.length);
        return out;
    }

    private static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
        System.out.println("  ok  " + what);
    }
}
