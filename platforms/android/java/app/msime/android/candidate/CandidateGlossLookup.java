package app.msime.android;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 一代候选的离线释义：随包的英文释义，加上装了的各离线语言，在调用线程上查，按候选记住。
 *
 * <p>键盘在主线程上、画这一代候选之前查（鸿蒙也在 UI 线程上查）：查一次只要一两毫秒，而交给工作线程再排回主线程要十几到几十毫秒，
 * 这段时间里候选先画成没有释义的样子，释义随后才出现，停顿重排换代时又被清掉一次，看上去一闪一闪。共享层把打开过的词库留在查询线程上，
 * 所以只在主线程上查，词库也只在主线程上留一份；{@link #warmUp} 在键盘显示后的空闲时刻先打开它们，打字时的第一次查询就不用等打开。
 *
 * <p>释义属于候选本身（文字和来源），不属于哪一代：停顿重排、在线行到达换了代，候选还是那些词。所以按语言、来源和文字记住查过的回答
 * （没有释义也记），新的一代只查没见过的候选，全都见过时不查。每种语言最多记 {@link #CAPACITY} 条，最久没用的先丢。
 * 宿主在释义设置、词典或资源变了、键盘显示（设置页可能改了自定义译文）、内存吃紧时调 {@link #clear}；无痕时不记（{@link #setRemembering}）。
 */
public final class CandidateGlossLookup {
    /** 一行候选：文字和共享层的来源编号。 */
    public record Row(String text, int source) {}

    /**
     * 原生查询（`NativeClient.candidateGlosses`，宿主负责拼请求、解码、核对代次）：{@code language} 为 null 时查随包的英文释义。
     * 返回有释义的那些候选（文字到释义）；读不出来或代次不对时抛出。
     */
    public interface Native {
        Map<String, String> glosses(long generation, List<Row> rows, String language) throws Exception;
    }

    /**
     * {@code english} 是这一代各候选的英文释义（按候选顺序），画不出来时（目标语言不是英语、目标语言的离线词典装着）不查，为 null；
     * {@code offline} 在要与在线译文合并时才有（目标语言含英语，或装了非英文离线词典），按语言放每个候选的释义，没有时为 null。
     */
    public record Found(Map<String, String> english, Map<String, Map<String, String>> offline) {}

    /** 每种语言记住的候选数：一代最多问 128 个，留几代的量；一条几百字节，一种语言一两百 KB。 */
    static final int CAPACITY = 512;
    /** 预热用的候选：只为让共享层打开词库，结果不用。 */
    private static final Row WARM_UP = new Row("你好", 0);
    private static final String ENGLISH = "en";

    private final Native lookup;
    private final int capacity;
    private final Map<String, CandidateLookupMemo<Row>> remembered = new HashMap<>();
    private boolean remembering = true;
    private long nativeLookups;

    public CandidateGlossLookup(Native lookup) {
        this(lookup, CAPACITY);
    }

    CandidateGlossLookup(Native lookup, int capacity) {
        this.lookup = lookup;
        this.capacity = capacity;
    }

    /** 无痕（隐私模式、不许学习的输入框）时传 false：不记，并丢掉已记的；之后每一代照常查。 */
    public void setRemembering(boolean remembering) {
        this.remembering = remembering;
        for (CandidateLookupMemo<Row> memo : remembered.values()) memo.setRemembering(remembering);
    }

    /** 丢掉记住的回答，下一次照常查。 */
    public void clear() {
        remembered.clear();
    }

    /** 调过几次原生查询（冒烟用）。 */
    public long nativeLookups() {
        return nativeLookups;
    }

    /**
     * 查一代候选。{@code targets} 是装了离线词典的非英文目标语言，{@code englishTarget} 为真时英文释义也记进合并用的表。
     * 只查画得出来的语言：候选下面只画目标语言（安卓只有一个），英文释义只在目标语言是英语、或目标语言的离线词典没装（这时退回画英文）时才用得上。
     * 某种语言的词典缺了或读不懂，只少那一种，其余照常；要查的英文释义读不出来时整次查询失败。
     */
    public Found lookUp(long generation, List<Row> rows, List<String> targets, boolean englishTarget)
            throws Exception {
        boolean englishShown = englishTarget || targets.isEmpty();
        Map<String, String> english = englishShown ? answer(generation, rows, null) : null;
        if (targets.isEmpty() && !englishTarget) return new Found(english, null);
        Map<String, Map<String, String>> offline = new HashMap<>(targets.size() + 1);
        if (english != null) offline.put(ENGLISH, english);
        for (String target : targets) {
            try {
                offline.put(target, answer(generation, rows, target));
            } catch (Exception error) {
                // One missing or unreadable dictionary leaves the other rows intact.
            }
        }
        return new Found(english, offline);
    }

    /**
     * 让共享层在调用线程上打开 {@code targets} 各语言和（{@code english} 为真时）英文的释义词库，与 {@link #lookUp} 查的语言相同；
     * 任何失败都不影响输入，也不记住什么。
     */
    public void warmUp(boolean english, List<String> targets) {
        List<Row> rows = List.of(WARM_UP);
        if (english) {
            try {
                nativeLookups++;
                lookup.glosses(0, rows, null);
            } catch (Exception error) {
                return;
            }
        }
        for (String target : targets) {
            try {
                nativeLookups++;
                lookup.glosses(0, rows, target);
            } catch (Exception error) {
                // A missing dictionary is opened (and fails) on the first real lookup instead.
            }
        }
    }

    /** 一种语言下这一代各候选的释义（按候选顺序，只含有释义的）：记住的直接用，没见过的一次查完再记下。 */
    private Map<String, String> answer(long generation, List<Row> rows, String language) throws Exception {
        String key = language == null ? ENGLISH : language;
        CandidateLookupMemo<Row> known = remembered.computeIfAbsent(key, ignored -> {
            CandidateLookupMemo<Row> memo = new CandidateLookupMemo<>(capacity);
            memo.setRemembering(remembering);
            return memo;
        });
        List<Row> missing = known.unseen(rows);
        Map<String, String> answered = Map.of();
        if (!missing.isEmpty()) {
            nativeLookups++;
            answered = lookup.glosses(generation, missing, language);
            for (Row row : missing) known.put(row, answered.get(row.text()));
        }
        LinkedHashMap<String, String> glosses = new LinkedHashMap<>();
        for (Row row : rows) {
            String gloss = known.get(row);
            if (gloss == null) gloss = answered.getOrDefault(row.text(), CandidateLookupMemo.NONE);
            if (!gloss.isEmpty()) glosses.putIfAbsent(row.text(), gloss);
        }
        return glosses;
    }
}
