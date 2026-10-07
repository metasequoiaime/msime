import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import java.lang.reflect.Method;

public final class SyncSignalsSmoke {
    public static void main(String[] arguments) throws Exception {
        check(SyncSwitch.SECTIONS.size() == 4, "settings, phrases, skins and dictionary");
        for (String section : SyncSwitch.SECTIONS) check(SyncSwitch.validSection(section), section);
        check(!SyncSwitch.validSection("tokens"), "an unknown section is refused");
        check(!SyncSwitch.validSection(null), "a missing section is refused");
        check(SyncSwitch.validLoginKind("google") && SyncSwitch.validLoginKind("apple")
            && SyncSwitch.validLoginKind("email"), "the three real-account sign-ins");
        check(!SyncSwitch.validLoginKind("anonymous"), "the anonymous account never syncs");

        Method cursorKey = SyncSwitch.class.getDeclaredMethod("cursorKey", String.class);
        cursorKey.setAccessible(true);
        Method generationKey = SyncSwitch.class.getDeclaredMethod("generationKey", String.class);
        generationKey.setAccessible(true);
        Method cleanKey = SyncSwitch.class.getDeclaredMethod("cleanKey", String.class);
        cleanKey.setAccessible(true);
        check("cursor_phrases".equals(cursorKey.invoke(null, "phrases")), "cursor key per section");
        check("gen_skins".equals(generationKey.invoke(null, "skins")), "generation key per section");
        check("clean_skins".equals(cleanKey.invoke(null, "skins")), "cleared-at key per section");
        try {
            generationKey.invoke(null, "../escape");
            throw new AssertionError("an unknown section must not produce a key");
        } catch (java.lang.reflect.InvocationTargetException expected) {
            check(expected.getCause() instanceof IllegalArgumentException, "unknown section is an argument error");
        }

        Method stateOf = SyncSignals.class.getDeclaredMethod("stateOf", boolean.class, String.class);
        stateOf.setAccessible(true);
        SyncSignals.State on = (SyncSignals.State) stateOf.invoke(null, true, "email");
        check(on.enabled() && "email".equals(on.loginKind()), "an enabled real account reads as on");
        SyncSignals.State stray = (SyncSignals.State) stateOf.invoke(null, true, "");
        check(!stray.enabled(), "a switch without an account reads as off");
        SyncSignals.State unknown = (SyncSignals.State) stateOf.invoke(null, true, "anonymous");
        check(!unknown.enabled() && unknown.loginKind().isEmpty(), "an unknown login kind reads as off");
        System.out.println("Android sync signals passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
