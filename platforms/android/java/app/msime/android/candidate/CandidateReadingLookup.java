package app.msime.android;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 候选的离线读音（「显示读音」）：英文候选的音标、释义第一个词的读音，在调用线程上查，按词记住。
 *
 * <p>和释义一样在主线程上、画这一代之前查，读音和候选同一帧出现。键是 {@link CandidatePronunciationPolicy#key}（语言加要读的那段文字），
 * 与哪一代无关，查过的（没有读音也记）不再查。共享层每次查询都重新打开读音词库，所以只把没见过的交给它。
 * 读音资源包（{@code supplementary}）换了就作废；宿主在读音设置变了、键盘显示、内存吃紧时调 {@link #clear}；无痕时不记。
 */
public final class CandidateReadingLookup {
    /** 原生查询（`NativeClient.candidatePronunciations`，宿主拼请求、解码、核对代次）：返回有读音的那些键（键到读音）。 */
    public interface Native {
        Map<String, String> readings(long generation, List<String> keys, String supplementary) throws Exception;
    }

    private final Native lookup;
    private final CandidateLookupMemo<String> remembered;
    private String supplementary;
    private long nativeLookups;

    public CandidateReadingLookup(Native lookup) {
        this(lookup, CandidateGlossLookup.CAPACITY);
    }

    CandidateReadingLookup(Native lookup, int capacity) {
        this.lookup = lookup;
        this.remembered = new CandidateLookupMemo<>(capacity);
    }

    public void setRemembering(boolean remembering) {
        remembered.setRemembering(remembering);
    }

    public void clear() {
        remembered.clear();
    }

    /** 调过几次原生查询（冒烟用）。 */
    public long nativeLookups() {
        return nativeLookups;
    }

    /** 这一代各键的读音（按键的顺序，只含有读音的）：记住的直接用，没见过的一次查完再记下。查不到时抛出，调用方保持原样。 */
    public Map<String, String> readings(long generation, List<String> keys, String supplementary) throws Exception {
        if (!supplementary.equals(this.supplementary)) {
            remembered.clear();
            this.supplementary = supplementary;
        }
        List<String> missing = remembered.unseen(keys);
        Map<String, String> answered = Map.of();
        if (!missing.isEmpty()) {
            nativeLookups++;
            answered = lookup.readings(generation, missing, supplementary);
            for (String key : missing) remembered.put(key, answered.get(key));
        }
        LinkedHashMap<String, String> readings = new LinkedHashMap<>();
        for (String key : keys) {
            String reading = remembered.get(key);
            if (reading == null) reading = answered.getOrDefault(key, CandidateLookupMemo.NONE);
            if (!reading.isEmpty()) readings.putIfAbsent(key, reading);
        }
        return readings;
    }
}
