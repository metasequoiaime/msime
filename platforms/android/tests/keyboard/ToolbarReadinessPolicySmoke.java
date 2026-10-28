package app.msime.android;

import app.msime.android.ToolbarReadinessPolicy.LanguageTap;
import app.msime.android.ToolbarReadinessPolicy.PanelTap;
import app.msime.android.ToolbarReadinessPolicy.Session;

/** 引擎会话就绪前后工具栏入口与「中」键的可用性（#10086）。 */
public final class ToolbarReadinessPolicySmoke {
    public static void main(String[] args) {
        check(ToolbarReadinessPolicy.session(true, false) == Session.READY, "a live session is ready");
        check(ToolbarReadinessPolicy.session(true, true) == Session.READY, "a live session wins over a stale pending flag");
        check(ToolbarReadinessPolicy.session(false, true) == Session.STARTING, "a scheduled startup is starting");
        check(ToolbarReadinessPolicy.session(false, false) == Session.UNAVAILABLE, "no session and no startup");

        // 表情、常用语：会话在建时就能点，只看资源目录、偏好目录和输入连接。
        check(ToolbarReadinessPolicy.emojiEnabled(Session.STARTING, true, true), "emoji opens while the session starts");
        check(ToolbarReadinessPolicy.emojiEnabled(Session.READY, true, true), "emoji opens with a session");
        check(!ToolbarReadinessPolicy.emojiEnabled(Session.STARTING, true, false), "emoji needs the resources directory");
        check(!ToolbarReadinessPolicy.emojiEnabled(Session.STARTING, false, true), "emoji needs an input connection");
        check(!ToolbarReadinessPolicy.emojiEnabled(Session.UNAVAILABLE, true, true), "no emoji in sessionless editors");
        check(ToolbarReadinessPolicy.phraseEnabled(Session.STARTING, true), "phrases open while the session starts");
        check(!ToolbarReadinessPolicy.phraseEnabled(Session.STARTING, false), "phrases need the preferences directory");
        check(!ToolbarReadinessPolicy.phraseEnabled(Session.UNAVAILABLE, true), "no phrases in sessionless editors");

        // 皮肤、输入方式：会话在建时可点但推迟，建好后要等偏好。
        check(ToolbarReadinessPolicy.preferencePanelTap(Session.STARTING, false) == PanelTap.DEFER,
            "a skin or scheme tap during startup is deferred");
        check(ToolbarReadinessPolicy.preferencePanelTap(Session.STARTING, true) == PanelTap.DEFER,
            "a stale snapshot never opens a panel before the session");
        check(ToolbarReadinessPolicy.preferencePanelTap(Session.READY, true) == PanelTap.OPEN, "ready panels open");
        check(ToolbarReadinessPolicy.preferencePanelTap(Session.READY, false) == PanelTap.IGNORE,
            "a session without preferences still waits for the reloader");
        check(ToolbarReadinessPolicy.preferencePanelTap(Session.UNAVAILABLE, true) == PanelTap.IGNORE,
            "sessionless editors ignore the tap");
        check(ToolbarReadinessPolicy.preferencePanelEnabled(Session.STARTING, false), "not greyed out while starting");
        check(!ToolbarReadinessPolicy.preferencePanelEnabled(Session.READY, false), "greyed out while preferences load");
        check(!ToolbarReadinessPolicy.preferencePanelEnabled(Session.UNAVAILABLE, false), "greyed out without a session");
        check(!ToolbarReadinessPolicy.deferredTapReady(Session.STARTING, true), "deferred taps wait for the session");
        check(!ToolbarReadinessPolicy.deferredTapReady(Session.READY, false), "deferred taps wait for preferences");
        check(ToolbarReadinessPolicy.deferredTapReady(Session.READY, true), "deferred taps run once both are ready");

        // 「中」键。
        check(ToolbarReadinessPolicy.languageKeyActive(Session.STARTING), "the language key is live while starting");
        check(!ToolbarReadinessPolicy.languageKeyActive(Session.UNAVAILABLE), "the language key is dimmed without a session");
        check(ToolbarReadinessPolicy.languageTap(Session.READY, true, true, false) == LanguageTap.ENGINE,
            "a ready session handles the tap");
        check(ToolbarReadinessPolicy.languageTap(Session.UNAVAILABLE, false, false, false) == LanguageTap.IGNORE,
            "a sessionless editor ignores the tap");
        check(ToolbarReadinessPolicy.languageTap(Session.STARTING, false, false, false) == LanguageTap.HOST_TOGGLE,
            "Chinese to English flips on the host");
        check(ToolbarReadinessPolicy.languageTap(Session.STARTING, false, true, false) == LanguageTap.HOST_TOGGLE,
            "English to Chinese flips on the host");
        check(ToolbarReadinessPolicy.languageTap(Session.STARTING, false, false, true) == LanguageTap.HOST_TOGGLE,
            "without cycling another language keyboard only flips English");
        check(ToolbarReadinessPolicy.languageTap(Session.STARTING, true, false, false) == LanguageTap.HOST_TOGGLE,
            "cycling from Chinese always enters English");
        check(ToolbarReadinessPolicy.languageTap(Session.STARTING, true, true, false) == LanguageTap.DEFER,
            "cycling from English needs the enabled language keyboards");
        check(ToolbarReadinessPolicy.languageTap(Session.STARTING, true, false, true) == LanguageTap.DEFER,
            "cycling from another language needs the enabled language keyboards");
        System.out.println("Android toolbar readiness policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
