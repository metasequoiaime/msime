package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 中英键点一下切到哪里（#6648）。
 *
 * <p>默认（「中英键轮换其他语言」关着）和原来一样只在中文和英文之间来回。打开后按「中 → 英 → 已启用的其他语言键盘（按启用顺序）→ 中」轮换，和高德输入法的中英日依次切换一致；没有启用任何其他语言键盘时仍只切中英。
 *
 * <p>轮换按语言走，不按入口走：日语 9 键和 26 键都启用时只轮到其中一个，和中文入口同一种键盘（9 键或 26 键）的优先，没有就取启用顺序里的第一个。
 *
 * <p>当前位置按状态判断：英文直输开着算「英」，不管它是从哪个方案切过去的；否则当前方案是其他语言就算那一种，其余算「中」。其他语言之后不会再回到「英」，所以轮换总能回到中文。回到中文时，从中文方案切到英文的直接关掉英文；从其他语言回来的切回进入其他语言之前用的那个中文入口。
 */
public final class LanguageKeyCyclePolicy {
    private LanguageKeyCyclePolicy() {}

    /** 点一下要做的事。 */
    public enum Kind {
        /** 方案切换正在保存，忽略再次点按。 */
        BUSY,
        /** 打开英文直输（与原来的中英切换相同）。 */
        ENGLISH,
        /** 关掉英文直输，方案不变（与原来的中英切换相同）。 */
        CHINESE_TOGGLE,
        /** 切到 {@link Target#scheme()} 这个方案（其他语言键盘，或回到中文时的中文入口）。 */
        SCHEME
    }

    public record Target(Kind kind, KeyboardScheme scheme) {}

    /**
     * @param cycle 设置「中英键轮换其他语言」是否打开
     * @param selected 当前方案入口
     * @param english 英文直输是否开着
     * @param available 键盘此刻能切到的方案入口（已启用、词库也已就绪），按启用顺序
     * @param chineseReturn 从其他语言回到中文时切回的入口
     */
    public static Target next(boolean cycle, KeyboardScheme selected, boolean english,
            List<KeyboardScheme> available, KeyboardScheme chineseReturn) {
        return next(cycle, false, selected, english, available, chineseReturn);
    }

    /** 与上一个点按仍在异步保存方案时保持状态不变。 */
    public static Target next(boolean cycle, boolean busy, KeyboardScheme selected, boolean english,
            List<KeyboardScheme> available, KeyboardScheme chineseReturn) {
        if (busy) return new Target(Kind.BUSY, null);
        List<KeyboardScheme> others = otherLanguages(available,
            chineseReturn == null ? null : chineseReturn.touchKeyboardLayout());
        if (!cycle || others.isEmpty()) {
            return new Target(english ? Kind.CHINESE_TOGGLE : Kind.ENGLISH, null);
        }
        boolean inOther = selected != null && selected.otherLanguage();
        if (english) return new Target(Kind.SCHEME, others.get(0));
        if (!inOther) return new Target(Kind.ENGLISH, null);
        int index = -1;
        for (int candidate = 0; candidate < others.size(); candidate++) {
            if (others.get(candidate).language().equals(selected.language())) index = candidate;
        }
        if (index >= 0 && index + 1 < others.size()) return new Target(Kind.SCHEME, others.get(index + 1));
        return new Target(Kind.SCHEME, chineseReturn);
    }

    /** 中英键上显示的字：英文直输时是「英」；打开轮换且停在其他语言键盘上时是那个方案的字（あ、한、越、藏）；其余是「中」。 */
    public static String label(boolean cycle, KeyboardScheme selected, boolean english,
            List<KeyboardScheme> available) {
        if (english) return "英";
        if (cycle && selected != null && selected.otherLanguage() && !otherLanguages(available, null).isEmpty())
            return selected.glyph();
        return "中";
    }

    /** 写「要切到哪里」的键（日语 9 键左列的语言键）的键面：英文是「英」，回到中文是「中」，其他语言是它的字。 */
    public static String targetLabel(Target target) {
        if (target.kind() == Kind.ENGLISH) return "英";
        if (target.kind() == Kind.CHINESE_TOGGLE || target.scheme() == null || !target.scheme().otherLanguage())
            return "中";
        return target.scheme().glyph();
    }

    /** 读屏念的这一下会切到哪里；轮换关着时与原来的两句相同。 */
    public static String description(Target target) {
        if (target.kind() == Kind.ENGLISH) return "切换到英文输入";
        if (target.kind() == Kind.CHINESE_TOGGLE || target.scheme() == null) return "切换到所选输入方案";
        return "切换到" + target.scheme().title();
    }

    /** 读屏念的当前状态：英文输入、中文输入，或打开轮换时停在的其他语言键盘的名字。 */
    public static String stateDescription(boolean cycle, KeyboardScheme selected, boolean english,
            List<KeyboardScheme> available) {
        if (english) return "英文输入";
        if (cycle && selected != null && selected.otherLanguage() && !otherLanguages(available, null).isEmpty())
            return selected.title();
        return "中文输入";
    }

    /**
     * 从其他语言回到中文时切回的入口：最近一次用过的中文入口还能用就是它；否则是偏好 `last_chinese_scheme` 对应的第一个可用入口，再否则是第一个可用的中文入口；都没有时用 `fallback`。
     */
    public static KeyboardScheme chineseReturn(KeyboardScheme lastChinese, String lastChineseEngineScheme,
            List<KeyboardScheme> available, AppEdition edition, KeyboardScheme fallback) {
        if (lastChinese != null && !lastChinese.otherLanguage() && available.contains(lastChinese)) return lastChinese;
        KeyboardScheme first = null;
        for (KeyboardScheme candidate : available) {
            if (candidate.otherLanguage()) continue;
            if (first == null) first = candidate;
            if (candidate != KeyboardScheme.HANDWRITING
                    && candidate.engineScheme(edition).equals(lastChineseEngineScheme)) return candidate;
        }
        return first != null ? first : fallback;
    }

    /** 每种其他语言一个入口，按语言第一次出现的启用顺序；同一种语言有几个入口时取触屏布局是 `layout` 的那个，没有就取第一个。 */
    private static List<KeyboardScheme> otherLanguages(List<KeyboardScheme> available, String layout) {
        List<KeyboardScheme> others = new ArrayList<>(available == null ? 0 : available.size());
        if (available == null) return others;
        for (KeyboardScheme scheme : available) {
            if (!scheme.otherLanguage()) continue;
            int existing = -1;
            for (int index = 0; index < others.size(); index++) {
                if (others.get(index).language().equals(scheme.language())) existing = index;
            }
            if (existing < 0) others.add(scheme);
            else if (scheme.touchKeyboardLayout().equals(layout)
                    && !others.get(existing).touchKeyboardLayout().equals(layout)) others.set(existing, scheme);
        }
        return others;
    }
}
