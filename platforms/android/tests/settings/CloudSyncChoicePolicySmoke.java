import app.msime.android.CloudSyncChoicePolicy;
import java.util.List;

public final class CloudSyncChoicePolicySmoke {
    public static void main(String[] arguments) {
        CloudSyncChoicePolicy.Prompt first = CloudSyncChoicePolicy.prompt(false);
        check("云端已有同步数据".equals(first.title()), "first-run title: " + first.title());
        check(choices(first).equals(List.of("merge", "use_cloud")), "first run asks merge or use cloud: " + choices(first));

        // 云端被清空时多一个「关闭同步，保留本机」，交给 Rust 的是 choice:"disable"（迁移计划 5d）。
        CloudSyncChoicePolicy.Prompt cleared = CloudSyncChoicePolicy.prompt(true);
        check("云端数据已被清空".equals(cleared.title()), "cloud-cleared title: " + cleared.title());
        check(choices(cleared).equals(List.of("merge", "use_cloud", "disable")),
            "a cleared cloud also offers turning sync off: " + choices(cleared));
        check("关闭同步，保留本机".equals(cleared.options().get(2).label()), "the disable label");
        check(cleared.message().contains("关闭这台设备的同步"), "the message explains the third option");
        System.out.println("Android cloud sync choice policy passed");
    }

    private static List<String> choices(CloudSyncChoicePolicy.Prompt prompt) {
        return prompt.options().stream().map(CloudSyncChoicePolicy.Option::choice).toList();
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
