import app.msime.android.UserTranslationMemory;
import java.util.concurrent.atomic.AtomicLong;

/** 用户翻译服务记下的回答和退避：问过的词（包括回答为空的）不再问，失败的词和不可达的服务都在退避结束后才再试。 */
public final class UserTranslationMemorySmoke {
    public static void main(String[] args) {
        AtomicLong now = new AtomicLong(1_000);
        UserTranslationMemory memory = new UserTranslationMemory(now::get);
        check(memory.shouldAsk("en", "你好"), "an unknown word is asked");

        memory.answered("en", "你好", null);
        check(!memory.shouldAsk("en", "你好"), "a word answered with no gloss is not asked again");
        check("".equals(memory.answer("en", "你好")), "an empty answer is remembered as empty");
        check(memory.shouldAsk("ja", "你好"), "the empty answer belongs to one target language");
        now.addAndGet(UserTranslationMemory.BACKOFF_MILLIS * 10);
        check(!memory.shouldAsk("en", "你好"), "an empty answer does not expire with the backoff");

        check(memory.answer("en", "世界") == null, "a word never asked has no answer");
        memory.answered("en", "世界", "world");
        check(!memory.shouldAsk("en", "世界") && "world".equals(memory.answer("en", "世界")),
            "a gloss the store discarded is answered again without the network");

        memory.failed("en", "失败");
        check(!memory.shouldAsk("en", "失败"), "a failed word waits for the backoff");
        check(memory.shouldAsk("en", "别的"), "one failed word does not stop the others");
        now.addAndGet(UserTranslationMemory.BACKOFF_MILLIS - 1);
        check(!memory.shouldAsk("en", "失败"), "the backoff lasts its whole interval");
        now.addAndGet(1);
        check(memory.shouldAsk("en", "失败"), "a failed word is asked again after the backoff");

        memory.unreachable();
        check(memory.serviceBackingOff() && !memory.shouldAsk("en", "别的"),
            "an unreachable service is not asked about any word");
        now.addAndGet(UserTranslationMemory.BACKOFF_MILLIS);
        check(!memory.serviceBackingOff() && memory.shouldAsk("en", "别的"),
            "the service is asked again after the backoff");

        // 「target|word」直接拼接时，目标语言里的分隔符会让两个不同的键撞在一起。
        memory.answered("en|x", "y", null);
        check(memory.shouldAsk("en", "x|y"), "keys of different targets and words do not collide");
        // 记下的回答有上限，满了丢最久没用的。
        UserTranslationMemory bounded = new UserTranslationMemory(now::get);
        for (int index = 0; index < 600; index++) bounded.answered("en", "词" + index, "w" + index);
        check(bounded.answer("en", "词0") == null && "w599".equals(bounded.answer("en", "词599")),
            "the remembered answers are bounded");
        System.out.println("Android user translation memory: negative cache and backoff passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
