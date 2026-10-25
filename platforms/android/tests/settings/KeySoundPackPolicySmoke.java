import app.msime.android.policy.KeySoundPackPolicy;
import app.msime.android.policy.KeySoundPackPolicy.Entry;
import app.msime.android.policy.KeySoundPackPolicy.Pack;
import java.util.Arrays;
import java.util.List;

public final class KeySoundPackPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        List<Entry> catalog = Arrays.asList(
            new Entry("twinkle", "小星星", "sound", "sequence"),
            new Entry("msime-music-lofi", "Lo-fi 午后", "music", null),
            new Entry("msime-bubble", "气泡", "sound", "keys"),
            new Entry("default", "清脆键盘", "sound", "keys"),
            new Entry("mine", "", "sound", "keys"),
            null,
            new Entry("", "空", "sound", "keys"));
        List<Pack> packs = KeySoundPackPolicy.choices(catalog);
        check(packs.equals(List.of(new Pack("default", "清脆键盘"), new Pack("msime-bubble", "气泡"), new Pack("mine", "mine"))),
            "only key sound packs, the default first and the rest in catalog order, unnamed ones by id: " + packs);
        check(KeySoundPackPolicy.choices(null).isEmpty(), "no catalog lists nothing");
        check(KeySoundPackPolicy.selected(null).equals("default") && KeySoundPackPolicy.selected("").equals("default"),
            "an unset preference is the built-in default pack");
        check(KeySoundPackPolicy.selected("msime-bubble").equals("msime-bubble"), "a chosen pack stays chosen");
        check(KeySoundPackPolicy.label(packs, "msime-bubble").equals("气泡"), "the row shows the pack name");
        check(KeySoundPackPolicy.label(packs, "gone").equals("gone"), "a missing pack shows its id");
        System.out.println("Android key sound pack choices passed");
    }
}
