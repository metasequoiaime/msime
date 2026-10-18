import app.msime.android.LocalVoiceModelPolicy;
import app.msime.android.LocalVoiceModelPolicy.Action;
import app.msime.android.LocalVoiceModelPolicy.Download;

/** The settings page's on-device model rows: which models are listed, what each row offers, how it is described. */
public final class LocalVoiceModelPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(LocalVoiceModelPolicy.visible(false, false) && LocalVoiceModelPolicy.visible(false, true),
            "phone models are listed");
        check(!LocalVoiceModelPolicy.visible(true, false), "a desktop-only model is not offered on a phone");
        check(LocalVoiceModelPolicy.visible(true, true), "an installed desktop-only model stays listed so it can be removed");

        check(LocalVoiceModelPolicy.action(false, Download.NONE) == Action.DOWNLOAD, "a missing model offers download");
        check(LocalVoiceModelPolicy.action(false, Download.FAILED) == Action.RETRY, "a failed download offers retry");
        check(LocalVoiceModelPolicy.action(false, Download.DOWNLOADING) == Action.CANCEL, "a running download can be cancelled");
        check(LocalVoiceModelPolicy.action(true, Download.DOWNLOADING) == Action.CANCEL
                && LocalVoiceModelPolicy.action(true, Download.VERIFYING) == Action.VERIFYING,
            "a running download wins over an older installed copy");
        check(LocalVoiceModelPolicy.action(true, Download.NONE) == Action.MANAGE
                && LocalVoiceModelPolicy.action(true, Download.FAILED) == Action.MANAGE,
            "an installed model is managed, even after a failed re-download");

        check(LocalVoiceModelPolicy.installedValue(true).equals("使用中")
                && LocalVoiceModelPolicy.installedValue(false).equals("已下载"), "installed rows say whether they are in use");

        check(LocalVoiceModelPolicy.summary("中英流式", 158_000_000L, "约 0.5 GB").equals("中英流式 · 需下载 158 MB · 运行时约 0.5 GB"),
            "the summary names size and memory");
        check(LocalVoiceModelPolicy.summary("", 0, "").isEmpty() && LocalVoiceModelPolicy.summary(null, 0, null).isEmpty(),
            "nothing known means an empty summary");
        check(LocalVoiceModelPolicy.summary("", 2_000_000L, " ").equals("需下载 2 MB"), "separators only between present parts");

        check(LocalVoiceModelPolicy.selected("/data/files/voice-models/a", " /data/files/voice-models/a "),
            "the chosen path matches after trimming");
        check(!LocalVoiceModelPolicy.selected("", "") && !LocalVoiceModelPolicy.selected(null, null)
                && !LocalVoiceModelPolicy.selected("/data/files/voice-models/a", "/data/files/voice-models/b"),
            "empty or different paths are not selected");

        check(LocalVoiceModelPolicy.selectionAfterRemoval(
                "/data/files/voice-models/a", "/data/files/voice-models/a").isEmpty(),
            "deleting the currently selected model clears the selection");
        check(LocalVoiceModelPolicy.selectionAfterRemoval(
                "/data/files/voice-models/a", "/data/files/voice-models/b")
                .equals("/data/files/voice-models/b"),
            "deleting an old model keeps a newer selection");
        check(LocalVoiceModelPolicy.selectionAfterRemoval(
                "/data/files/voice-models/a", "").isEmpty(),
            "deleting an unselected model keeps an empty selection");
        System.out.println("Android local voice models: listing, row actions, summary and selection passed");
    }
}
