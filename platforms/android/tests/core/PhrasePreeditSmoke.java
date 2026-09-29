import app.msime.android.PhrasePreeditPolicy;

public final class PhrasePreeditSmoke {
    static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
    }

    public static void main(String[] args) {
        // 已选的那一段领在读音前面，两处都一样。
        check("海滩paobu".equals(PhrasePreeditPolicy.composing("海滩", "paobu")),
              "the chosen piece leads the reading in the editor");
        check("海滩跑步".equals(PhrasePreeditPolicy.title("海滩", "跑步", false)),
              "and on the candidate bar");

        // 没有正在拼的词时，两处都与改动之前逐字节相同——这是绝大多数按键走的路。
        check("nihao".equals(PhrasePreeditPolicy.composing("", "nihao")), "no prefix, no change");
        check("nihao".equals(PhrasePreeditPolicy.title("", "nihao", false)), "no prefix on the bar");

        // 本地模式有自己的标题：那时不存在「正在拼的词」，把一段汉字接在模式名前面只会让人以为
        // 模式名变了。
        check("Emoji".equals(PhrasePreeditPolicy.title("海滩", "Emoji", true)),
              "a local mode keeps its own title");

        // 空值按空串处理，不炸也不写出字面上的 null。
        check("paobu".equals(PhrasePreeditPolicy.composing(null, "paobu")), "null prefix");
        check("海滩".equals(PhrasePreeditPolicy.composing("海滩", null)), "null reading");
        check("海滩".equals(PhrasePreeditPolicy.title("海滩", null, false)), "null title");

        System.out.println("Android phrase preedit: the chosen piece leads both surfaces passed");
    }
}
