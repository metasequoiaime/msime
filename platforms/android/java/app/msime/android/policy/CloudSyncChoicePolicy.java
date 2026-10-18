package app.msime.android;

import java.util.List;

/**
 * 云同步要问用户时那张选择表的标题、说明和选项（#8201 迁移计划 5d）。Rust 的 run 回答 needs_choice 时给出原因：first_run 是某一类第一次同步而云端已有数据，问「合并 / 使用云端」；cloud_cleared 是云端被清空（多半是在别处「删除云端数据」），多一个「关闭同步，保留本机」，用户选了以 trigger:"choice" 把选项交回 Rust。措辞与 iOS 的相同。
 *
 * <p>不碰 Android 视图和 org.json，所以能在 check-host 的冒烟里直接跑。
 */
public final class CloudSyncChoicePolicy {
    /** 一个选项：按钮上的字，以及交给 Rust 的 choice。 */
    public record Option(String label, String choice) {}

    /** 一张选择表。 */
    public record Prompt(String title, String message, List<Option> options) {}

    public static final String MERGE = "merge";
    public static final String USE_CLOUD = "use_cloud";
    /** 只回答「云端被清空」：关掉这台设备的同步，本机数据不动。 */
    public static final String DISABLE = "disable";

    private CloudSyncChoicePolicy() {}

    public static Prompt prompt(boolean cloudCleared) {
        if (!cloudCleared) {
            return new Prompt("云端已有同步数据", "合并会保留两边的内容，同一项以本机为准；使用云端会用云端的内容替换本机",
                List.of(new Option("合并", MERGE), new Option("使用云端", USE_CLOUD)));
        }
        return new Prompt("云端数据已被清空",
            "云端的同步数据可能已在其他设备上删除。合并或使用云端都会保留本机的内容并传回云端；也可以关闭这台设备的同步，本机数据保持不变",
            List.of(new Option("合并", MERGE), new Option("使用云端", USE_CLOUD), new Option("关闭同步，保留本机", DISABLE)));
    }
}
