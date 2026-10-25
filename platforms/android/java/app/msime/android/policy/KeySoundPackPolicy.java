package app.msime.android.policy;

import java.util.ArrayList;
import java.util.List;

/**
 * 设置里「音效」可选的按键音包：插件目录里 `kind = sound`、`mode = keys` 的包，内置默认包排第一，其余按目录给出的顺序；旋律包（`mode = sequence`）和背景音乐不在其中。偏好 `plugins.key_sound.pack` 指的包不在列表里（被删除或不是按键音包）时，行上照实显示它的 id，键盘那边会退回系统按键音。
 */
public final class KeySoundPackPolicy {
    /** 没选过音效时的包：随包内置的「清脆键盘」。 */
    public static final String DEFAULT_PACK = "default";

    /** 目录里的一项，只取这里要用的字段。 */
    public record Entry(String id, String name, String kind, String mode) {}

    /** 可选的一个按键音包。 */
    public record Pack(String id, String name) {}

    private KeySoundPackPolicy() {}

    public static List<Pack> choices(List<Entry> catalog) {
        List<Pack> packs = new ArrayList<>();
        if (catalog == null) return packs;
        for (Entry entry : catalog) {
            if (entry == null || entry.id() == null || entry.id().isEmpty()) continue;
            if (!"sound".equals(entry.kind()) || !"keys".equals(entry.mode())) continue;
            String name = entry.name() == null || entry.name().isEmpty() ? entry.id() : entry.name();
            Pack pack = new Pack(entry.id(), name);
            if (DEFAULT_PACK.equals(pack.id())) packs.add(0, pack); else packs.add(pack);
        }
        return packs;
    }

    /** 偏好里的包 id；空值按 {@link #DEFAULT_PACK}。 */
    public static String selected(String preference) {
        return preference == null || preference.isEmpty() ? DEFAULT_PACK : preference;
    }

    /** 行上显示的名字：列表里有就用包名，没有就显示 id。 */
    public static String label(List<Pack> choices, String id) {
        if (choices != null) {
            for (Pack pack : choices) if (pack.id().equals(id)) return pack.name();
        }
        return id;
    }
}
