package app.msime.android;

import java.util.Map;
import java.util.TreeMap;

/** 每日本地按键批次；只保留键位、按下时的来源和次数，不保留顺序、时间或文本。 */
public final class KeyPressBatch {
    /** 达到此次数就写出，不等 30 秒定时器。 */
    public static final int FLUSH_PRESSES = 256;

    /** 主线程交给工作线程的不可变批次，来源在按下时取得，不在写出时读取当前模式。 */
    public record Flush(String day, Map<String, Long> keys,
                        Map<TypingSource, Map<String, Long>> sources) {
        public Flush {
            keys = Map.copyOf(keys);
            Map<TypingSource, Map<String, Long>> copy = new TreeMap<>();
            sources.forEach((source, counts) -> copy.put(source, Map.copyOf(counts)));
            sources = Map.copyOf(copy);
        }

        /** 整批按键次数。 */
        public long presses() {
            long total = 0;
            for (long count : keys.values()) total += count;
            return total;
        }
    }

    private final TreeMap<String, Long> keys = new TreeMap<>();
    private final TreeMap<TypingSource, Map<String, Long>> sources = new TreeMap<>();
    private String day = "";
    private int presses;

    /** 兼容没有来源的调用，保留为未分类。 */
    public Flush add(String id, String today) {
        return add(id, today, TypingSource.UNKNOWN);
    }

    /** 跨午夜先交回旧日批次；语言切换留在同一批，不增加写盘次数。 */
    public Flush add(String id, String today, TypingSource source) {
        if (!KeyPressIds.isKnown(id) || today == null || today.isEmpty()) return null;
        Flush previous = day.equals(today) ? null : drain();
        day = today;
        keys.merge(id, 1L, Long::sum);
        sources.computeIfAbsent(source == null ? TypingSource.UNKNOWN : source,
            ignored -> new TreeMap<>()).merge(id, 1L, Long::sum);
        presses++;
        return previous;
    }

    /** 达到按键阈值时立即写出。 */
    public boolean full() { return presses >= FLUSH_PRESSES; }

    public boolean isEmpty() { return presses == 0; }

    /** 取出并清空当前批次，没有计数时返回 null。 */
    public Flush drain() {
        if (presses == 0) return null;
        Flush flush = new Flush(day, keys, sources);
        clear();
        return flush;
    }

    /** 关闭统计时丢弃整批，不补写。 */
    public void clear() {
        keys.clear();
        sources.clear();
        presses = 0;
        day = "";
    }
}
