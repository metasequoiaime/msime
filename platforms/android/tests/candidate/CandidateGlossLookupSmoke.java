package app.msime.android;

import app.msime.android.CandidateGlossLookup.Row;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** 一代候选的离线释义查询与按候选记住：用假的原生查询驱动，只看查了什么、记住了什么、交回什么。用到包内的容量构造，所以在包里。 */
public final class CandidateGlossLookupSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class Fake implements CandidateGlossLookup.Native {
        final List<String> calls = new ArrayList<>();
        final Map<String, Map<String, String>> dictionaries = new HashMap<>();
        final List<String> failing = new ArrayList<>();

        @Override public Map<String, String> glosses(long generation, List<Row> rows, String language) {
            String key = language == null ? "en" : language;
            StringBuilder asked = new StringBuilder(key).append(':');
            for (Row row : rows) asked.append(row.text()).append(',');
            calls.add(asked.toString());
            if (failing.contains(key)) throw new IllegalStateException("synthetic missing dictionary");
            Map<String, String> dictionary = dictionaries.getOrDefault(key, Map.of());
            Map<String, String> answer = new HashMap<>();
            for (Row row : rows) {
                String gloss = dictionary.get(row.text());
                if (gloss != null) answer.put(row.text(), gloss);
            }
            return answer;
        }
    }

    private static List<Row> rows(String... texts) {
        List<Row> rows = new ArrayList<>();
        for (String text : texts) rows.add(new Row(text, 0));
        return rows;
    }

    public static void main(String[] args) throws Exception {
        Fake fake = new Fake();
        fake.dictionaries.put("en", Map.of("你好", "hello", "世界", "world"));
        fake.dictionaries.put("ja", Map.of("你好", "こんにちは"));
        fake.dictionaries.put("fr", Map.of("你好", "bonjour"));
        CandidateGlossLookup lookup = new CandidateGlossLookup(fake);

        CandidateGlossLookup.Found plain = lookup.lookUp(7, rows("你好", "世界", "啊"), List.of(), false);
        check(plain.offline() == null, "没有离线语言、目标也不含英语时不做合并表");
        check(plain.english().equals(Map.of("你好", "hello", "世界", "world")), "交回有释义的候选");
        check(fake.calls.equals(List.of("en:你好,世界,啊,")), "第一次整代查一次");

        fake.calls.clear();
        CandidateGlossLookup.Found again = lookup.lookUp(8, rows("世界", "你好", "啊"), List.of(), false);
        check(fake.calls.isEmpty(), "换了代、词没变（停顿重排）时不再查，没有释义的也记得");
        check(again.english().keySet().iterator().next().equals("世界"), "按这一代的候选顺序交回");

        lookup.lookUp(9, rows("你好", "新词"), List.of(), false);
        check(fake.calls.equals(List.of("en:新词,")), "只查没见过的候选");

        fake.calls.clear();
        lookup.lookUp(10, List.of(new Row("你好", 6)), List.of(), false);
        check(fake.calls.equals(List.of("en:你好,")), "同样的文字换了来源要另查：来源决定答不答");

        fake.calls.clear();
        CandidateGlossLookup.Found japanese = lookup.lookUp(11, rows("你好", "世界"), List.of("ja"), false);
        check(japanese.english() == null && !japanese.offline().containsKey("en"),
            "目标语言不是英语、离线词典装着时不查英文：画不出来");
        check(japanese.offline().get("ja").equals(Map.of("你好", "こんにちは")), "目标语言的释义");
        check(fake.calls.equals(List.of("ja:你好,世界,")), "每种语言各记各的");

        fake.calls.clear();
        CandidateGlossLookup.Found both = lookup.lookUp(12, rows("你好"), List.of("fr"), true);
        check(both.english().equals(Map.of("你好", "hello")) && both.offline().get("en").equals(both.english()),
            "目标语言含英语时英文释义也查、也进合并表");
        check(fake.calls.equals(List.of("fr:你好,")), "英文记得，只查法语");

        fake.calls.clear();
        fake.failing.add("ja");
        lookup.clear();
        CandidateGlossLookup.Found missing = lookup.lookUp(13, rows("你好"), List.of("ja"), false);
        check(!missing.offline().containsKey("ja"), "缺词典的语言只少它一种");
        fake.calls.clear();
        lookup.lookUp(14, rows("你好"), List.of("ja"), false);
        check(fake.calls.equals(List.of("ja:你好,")), "查失败的不记，下一代再试");
        fake.failing.remove("ja");

        fake.failing.add("en");
        boolean failed = false;
        try {
            lookup.lookUp(15, rows("没见过"), List.of(), false);
        } catch (IllegalStateException expected) {
            failed = true;
        }
        check(failed, "要画的英文释义读不出来时整次查询失败，由调用方保持原视图");
        fake.failing.remove("en");

        fake.calls.clear();
        lookup.clear();
        lookup.lookUp(16, rows("你好"), List.of(), false);
        check(fake.calls.equals(List.of("en:你好,")), "清掉之后照常查");

        fake.calls.clear();
        lookup.setRemembering(false);
        lookup.lookUp(17, rows("你好"), List.of(), false);
        lookup.lookUp(18, rows("你好"), List.of(), false);
        check(fake.calls.equals(List.of("en:你好,", "en:你好,")), "无痕时不记：每一代都查，且清掉了之前记的");
        lookup.setRemembering(true);

        CandidateGlossLookup small = new CandidateGlossLookup(fake, 2);
        fake.calls.clear();
        small.lookUp(1, rows("甲", "乙", "丙"), List.of(), false);
        small.lookUp(2, rows("甲"), List.of(), false);
        check(fake.calls.equals(List.of("en:甲,乙,丙,", "en:甲,")), "超出容量时最久没用的先丢");

        fake.calls.clear();
        lookup.warmUp(false, List.of("ja", "fr"));
        check(fake.calls.equals(List.of("ja:你好,", "fr:你好,")), "预热只打开画得出来的语言");
        fake.calls.clear();
        fake.failing.add("en");
        lookup.warmUp(true, List.of("ja"));
        check(fake.calls.equals(List.of("en:你好,")), "英文都打不开时预热就此停下");
        System.out.println("CandidateGlossLookupSmoke: passed");
    }
}
