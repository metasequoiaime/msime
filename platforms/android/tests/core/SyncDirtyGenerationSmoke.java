package app.msime.android;

import java.util.Set;

public final class SyncDirtyGenerationSmoke {
    public static void main(String[] arguments) {
        MemoryPreferences store = new MemoryPreferences();
        String section = SyncSwitch.PHRASES;

        SyncSwitch.markDirty(store, section);
        check(!SyncSwitch.dirty(store, section), "sync off: nothing is marked");
        store.edit().putBoolean(SyncSwitch.KEY_ENABLED, true).apply();

        SyncSwitch.markDirty(store, section);
        check(SyncSwitch.dirty(store, section), "a local edit marks the section");
        long before = SyncSwitch.generation(store, section);
        check(SyncSwitch.clearDirtyIf(store, section, before), "no edit during the round: cleared");
        check(!SyncSwitch.dirty(store, section), "cleared section reads clean");

        // 上传期间用户又改了一条：清标记必须落空，下一轮才会把它传上去。
        SyncSwitch.markDirty(store, section);
        long snapshot = SyncSwitch.generation(store, section);
        SyncSwitch.markDirty(store, section);
        check(!SyncSwitch.clearDirtyIf(store, section, snapshot), "an edit during the round keeps the mark");
        check(SyncSwitch.dirty(store, section), "the edit made during the round is still pending");

        // 同步自己写了两次本机（每次都会经 CommonPhrasesStore 把代数加一），期间没有别的改动：照常清掉。
        long start = SyncSwitch.generation(store, section);
        SyncSwitch.markDirty(store, section);
        SyncSwitch.markDirty(store, section);
        check(SyncSwitch.clearDirtyIf(store, section, start + 2), "the round's own writes are accounted for");
        check(!SyncSwitch.dirty(store, section), "clean after the round's own writes");

        // 自己写了一次，用户又改了一次：仍然留着。
        start = SyncSwitch.generation(store, section);
        SyncSwitch.markDirty(store, section);
        SyncSwitch.markDirty(store, section);
        check(!SyncSwitch.clearDirtyIf(store, section, start + 1), "a user edit beside the round's write survives");
        SyncSwitch.clearDirty(store, section);
        check(!SyncSwitch.dirty(store, section), "unconditional clear still works");

        for (String other : Set.of(SyncSwitch.SETTINGS, SyncSwitch.SKINS, SyncSwitch.DICTIONARY)) {
            check(!SyncSwitch.dirty(store, other), "sections are independent: " + other);
        }
        System.out.println("Android sync dirty generation passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
