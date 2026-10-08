import app.msime.android.McpUploadSwitchPolicy;
import app.msime.android.McpUploadSwitchPolicy.Action;

/** 开发者选项里 MCP 上传开关的显示与点按（#5539：等待确认时开关显示为关闭，点了看起来毫无反应）。 */
public final class McpUploadSwitchPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 没有快照：开关关闭，点开进入确认。
        check(!McpUploadSwitchPolicy.checked(false, false), "nothing uploaded and nothing pending shows off");
        check(McpUploadSwitchPolicy.onToggle(true, false) == Action.START_CONFIRM, "switching on asks for confirmation");

        // 等待确认：开关显示为打开，所以下一次点按是关闭，也就是取消。
        check(McpUploadSwitchPolicy.checked(false, true), "a pending confirmation shows on");
        boolean next = !McpUploadSwitchPolicy.checked(false, true);
        check(McpUploadSwitchPolicy.onToggle(next, true) == Action.CANCEL_CONFIRM,
            "tapping the switch while confirming cancels instead of doing nothing");

        // 已上传：开关打开，关闭是删除；重新上传的确认期间关闭只是取消，不删快照。
        check(McpUploadSwitchPolicy.checked(true, false), "an uploaded snapshot shows on");
        check(McpUploadSwitchPolicy.onToggle(false, false) == Action.DELETE, "switching off an upload deletes it");
        check(McpUploadSwitchPolicy.checked(true, true), "re-uploading keeps the switch on");
        check(McpUploadSwitchPolicy.onToggle(false, true) == Action.CANCEL_CONFIRM,
            "switching off during a re-upload confirmation only cancels it");
        System.out.println("Android MCP upload switch passed");
    }
}
