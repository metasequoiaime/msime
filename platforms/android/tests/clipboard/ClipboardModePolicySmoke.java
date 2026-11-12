import app.msime.android.ClipboardModePolicy;
import java.io.File;

/** 访客剪贴板的目录隔离和回车上滑边界。 */
public final class ClipboardModePolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        File root = new File("/synthetic/msime-files");
        check(ClipboardModePolicy.directory(root, ClipboardModePolicy.Mode.MAIN).equals(root),
            "main mode keeps the normal store");
        File visitor = ClipboardModePolicy.directory(root, ClipboardModePolicy.Mode.VISITOR);
        check(visitor.equals(new File(root, ClipboardModePolicy.VISITOR_DIRECTORY)),
            "visitor mode uses a separate directory");
        check(!visitor.equals(root), "visitor mode never aliases the main store");
        check(ClipboardModePolicy.handledClipPreferenceKey(ClipboardModePolicy.Mode.MAIN)
                .equals("handled_clip_main"),
            "main mode has its own handled clipboard identity");
        check(ClipboardModePolicy.handledClipPreferenceKey(ClipboardModePolicy.Mode.VISITOR)
                .equals("handled_clip_visitor"),
            "visitor mode has its own handled clipboard identity");
        check(ClipboardModePolicy.legacyHandledClipPreferenceKey().equals("handled_clip"),
            "the pre-isolation handled clipboard identity remains a migration source");

        check(ClipboardModePolicy.isValidSwipe(50, 200, 50, 100, 100, 50, 500),
            "an upward swipe into the keyboard toggles");
        check(!ClipboardModePolicy.isValidSwipe(50, 200, 50, 190, 100, 50, 500),
            "a short hold is not a toggle");
        check(!ClipboardModePolicy.isValidSwipe(50, 200, 180, 100, 100, 50, 500),
            "a wide diagonal swipe is not a toggle");
        check(!ClipboardModePolicy.isValidSwipe(50, 200, 50, -1, 100, 50, 500),
            "a swipe outside the keyboard is not a toggle");
        check(ClipboardModePolicy.enterIcon(ClipboardModePolicy.Mode.MAIN, false)
                == ClipboardModePolicy.EnterIcon.RETURN,
            "main mode keeps the return icon");
        check(ClipboardModePolicy.enterIcon(ClipboardModePolicy.Mode.VISITOR, false)
                == ClipboardModePolicy.EnterIcon.RETURN,
            "restored visitor mode keeps the return icon");
        check(ClipboardModePolicy.enterIcon(ClipboardModePolicy.Mode.VISITOR, true)
                == ClipboardModePolicy.EnterIcon.TRASH,
            "visitor mode shows the trash icon only during the transition");
        System.out.println("ClipboardModePolicySmoke ok");
    }
}
