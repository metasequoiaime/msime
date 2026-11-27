package app.msime.android;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** 候选读音的查询与按词记住：用假的原生查询驱动。用到包内的容量构造，所以在包里。 */
public final class CandidateReadingLookupSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static final class Fake implements CandidateReadingLookup.Native {
        final List<String> calls = new ArrayList<>();
        final Map<String, String> table = new HashMap<>();
        boolean failing;

        @Override public Map<String, String> readings(long generation, List<String> keys, String supplementary) {
            calls.add(supplementary + ":" + String.join(",", keys));
            if (failing) throw new IllegalStateException("synthetic missing table");
            Map<String, String> answer = new HashMap<>();
            for (String key : keys) {
                String reading = table.get(key);
                if (reading != null) answer.put(key, reading);
            }
            return answer;
        }
    }

    public static void main(String[] args) throws Exception {
        Fake fake = new Fake();
        fake.table.put("en\nhello", "/həˈloʊ/");
        fake.table.put("ja\nこんにちは", "konnichiwa");
        CandidateReadingLookup lookup = new CandidateReadingLookup(fake);

        Map<String, String> first = lookup.readings(1, List.of("en\nhello", "en\nzzz", "ja\nこんにちは"), "");
        check(first.equals(Map.of("en\nhello", "/həˈloʊ/", "ja\nこんにちは", "konnichiwa")), "只交回有读音的键");
        check(fake.calls.equals(List.of(":en\nhello,en\nzzz,ja\nこんにちは")), "第一次整批查");

        fake.calls.clear();
        lookup.readings(2, List.of("ja\nこんにちは", "en\nzzz", "en\nhello"), "");
        check(fake.calls.isEmpty(), "换了代、词没变时不再查，没有读音的也记得");
        lookup.readings(3, List.of("en\nhello", "en\nnew"), "");
        check(fake.calls.equals(List.of(":en\nnew")), "只查没见过的");

        fake.calls.clear();
        lookup.readings(4, List.of("en\nhello"), "/packs");
        check(fake.calls.equals(List.of("/packs:en\nhello")), "读音资源包换了，记住的作废");

        fake.calls.clear();
        fake.failing = true;
        boolean failed = false;
        try {
            lookup.readings(5, List.of("en\nunseen"), "/packs");
        } catch (IllegalStateException expected) {
            failed = true;
        }
        check(failed, "查不到时抛出，调用方保持原样");
        fake.failing = false;
        lookup.readings(6, List.of("en\nunseen"), "/packs");
        check(fake.calls.size() == 2, "失败的不记，下一代再查");

        fake.calls.clear();
        lookup.clear();
        lookup.readings(7, List.of("en\nhello"), "/packs");
        check(fake.calls.size() == 1, "清掉之后照常查");

        fake.calls.clear();
        lookup.setRemembering(false);
        lookup.readings(8, List.of("en\nhello"), "/packs");
        lookup.readings(9, List.of("en\nhello"), "/packs");
        check(fake.calls.size() == 2, "无痕时不记，每一代都查");
        lookup.setRemembering(true);

        CandidateReadingLookup small = new CandidateReadingLookup(fake, 2);
        fake.calls.clear();
        small.readings(1, List.of("en\na", "en\nb", "en\nc"), "");
        small.readings(2, List.of("en\na"), "");
        check(fake.calls.size() == 2 && fake.calls.get(1).equals(":en\na"), "超出容量时最久没用的先丢");
        System.out.println("CandidateReadingLookupSmoke: passed");
    }
}
