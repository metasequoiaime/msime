import app.msime.android.AndroidLocalSettings;
import app.msime.android.ClipboardCapturePolicy;
import app.msime.android.ClipboardImagePolicy;
import app.msime.android.RecentClipboardSuggestion;

/** #5692：工具栏上的「最近复制」显示多久、什么时候不再出现，以及它的预览文字；#6781：复制的是图片时的同一套状态。 */
public final class RecentClipboardSuggestionSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        long copiedAt = 1_791_389_562_434L;
        String identity = ClipboardCapturePolicy.identity(copiedAt, "synthetic copied text");
        RecentClipboardSuggestion recent = new RecentClipboardSuggestion();
        check(recent.text(copiedAt) == null, "nothing copied, nothing shown");

        recent.offer(identity, "synthetic copied text", copiedAt);
        check("synthetic copied text".equals(recent.text(copiedAt + 1_000)), "a fresh copy is shown");
        check(recent.remainingMs(copiedAt + 1_000) == RecentClipboardSuggestion.WINDOW_MS - 1_000,
            "the row knows when to go away");
        check(recent.text(copiedAt + RecentClipboardSuggestion.WINDOW_MS + 1) == null,
            "the copy goes away after the window");
        check(recent.remainingMs(copiedAt + RecentClipboardSuggestion.WINDOW_MS + 1) == 0,
            "nothing left to wait for once expired");
        check("synthetic copied text".equals(recent.text(copiedAt - 5_000)),
            "a clock set back does not hide a fresh copy");

        // 用过或关掉之后，同一条再被看到也不出现；复制新的一条才出现。
        recent.dismiss();
        check(recent.text(copiedAt + 2_000) == null, "a dismissed copy is gone");
        recent.offer(identity, "synthetic copied text", copiedAt);
        check(recent.text(copiedAt + 2_000) == null, "the same clip is not offered again once dismissed");
        long later = copiedAt + 10_000;
        recent.offer(ClipboardCapturePolicy.identity(later, "another synthetic copy"), "another synthetic copy", later);
        check("another synthetic copy".equals(recent.text(later + 1)), "a new copy is offered again");
        recent.offer(ClipboardCapturePolicy.identity(later, "   "), "   ", later);
        check("another synthetic copy".equals(recent.text(later + 1)), "blank text is never offered");

        // #6781：复制的是图片时只记身份和类型；文字和图片同一时刻至多一条，后到的替换先到的。
        long imageAt = later + 20_000;
        String imageIdentity = ClipboardCapturePolicy.identity(imageAt,
            ClipboardImagePolicy.identityText("content://synthetic.provider/image/1", "image/png"));
        recent.offerImage(imageIdentity, "image/png", imageAt);
        check("image/png".equals(recent.imageMimeType(imageAt + 1)), "a fresh image copy is offered");
        check(recent.text(imageAt + 1) == null, "an image copy replaces the text copy");
        check(imageIdentity.equals(recent.identity(imageAt + 1)), "the service can check the image is still on the clipboard");
        check(recent.remainingMs(imageAt + 1_000) == RecentClipboardSuggestion.WINDOW_MS - 1_000,
            "an image copy expires like a text copy");
        check(recent.imageMimeType(imageAt + RecentClipboardSuggestion.WINDOW_MS + 1) == null
                && recent.identity(imageAt + RecentClipboardSuggestion.WINDOW_MS + 1) == null,
            "the image goes away after the window");
        recent.dismiss();
        check(recent.imageMimeType(imageAt + 1) == null, "a sent or dismissed image is gone");
        recent.offerImage(imageIdentity, "image/png", imageAt);
        check(recent.imageMimeType(imageAt + 1) == null, "the same image is not offered again once dismissed");
        long textAfterImage = imageAt + 5_000;
        recent.offerImage(ClipboardCapturePolicy.identity(textAfterImage,
            ClipboardImagePolicy.identityText("content://synthetic.provider/image/2", "image/jpeg")), "image/jpeg", textAfterImage);
        recent.offer(ClipboardCapturePolicy.identity(textAfterImage + 1, "synthetic text after image"),
            "synthetic text after image", textAfterImage + 1);
        check(recent.imageMimeType(textAfterImage + 2) == null
                && "synthetic text after image".equals(recent.text(textAfterImage + 2)),
            "a text copy replaces the image copy");
        recent.offerImage("synthetic-identity", " ", textAfterImage);
        check("synthetic text after image".equals(recent.text(textAfterImage + 2)), "an image without a type is never offered");

        check(RecentClipboardSuggestion.fresh(copiedAt, copiedAt + 59_000), "a minute-old copy is fresh");
        check(!RecentClipboardSuggestion.fresh(copiedAt, copiedAt + 61_000), "an older copy is not");
        check(!RecentClipboardSuggestion.fresh(0, copiedAt), "an unknown copy time is not taken as fresh");

        check("line one line two".equals(RecentClipboardSuggestion.preview("  line one\n\n  line two \t")),
            "the preview folds whitespace and line breaks into single spaces");
        String longText = "很长".repeat(40);
        String preview = RecentClipboardSuggestion.preview(longText);
        check(preview.endsWith("…") && preview.codePointCount(0, preview.length())
                == RecentClipboardSuggestion.PREVIEW_CODE_POINTS + 1,
            "a long copy is cut at the preview length with an ellipsis");
        check(!RecentClipboardSuggestion.preview("a".repeat(39) + " bcd").contains(" …"),
            "the cut never leaves a dangling space before the ellipsis");
        String emoji = RecentClipboardSuggestion.preview("🌲".repeat(50));
        check(!Character.isHighSurrogate(emoji.charAt(emoji.length() - 2)), "the cut never splits a surrogate pair");
        check("".equals(RecentClipboardSuggestion.preview(null)), "no text, no preview");

        // 剪贴板历史关着时不提供，也不为它读剪贴板；开着时听本地开关。
        check(!RecentClipboardSuggestion.enabled(false, true), "clipboard history off hides the recent copy");
        check(!RecentClipboardSuggestion.enabled(false, false), "both off, nothing shown");
        check(RecentClipboardSuggestion.enabled(true, true), "history on and the switch on shows it");
        check(!RecentClipboardSuggestion.enabled(true, false), "the local switch can still turn it off");

        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.CLIPBOARD_SUGGESTION);
        check(AndroidLocalSettings.defaults().bool(AndroidLocalSettings.CLIPBOARD_SUGGESTION),
            "the toolbar shows the last copy by default");
        check(!spec.synced, "the switch is local only: the sync field table does not know it");
        System.out.println("RecentClipboardSuggestionSmoke ok");
    }
}
