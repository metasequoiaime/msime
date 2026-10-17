import android.text.InputType;
import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.CloudClipboardApi;
import app.msime.android.CloudClipboardPanelPolicy;
import app.msime.android.CloudClipboardPanelPolicy.Status;
import app.msime.android.CloudClipboardPanelPolicy.Tab;

public final class CloudClipboardPanelPolicySmoke {
    public static void main(String[] args) {
        int text = InputType.TYPE_CLASS_TEXT;
        check(CloudClipboardPanelPolicy.cloudAllowed(text, true), "an ordinary text field allows the cloud half");
        check(!CloudClipboardPanelPolicy.cloudAllowed(
            text | InputType.TYPE_TEXT_VARIATION_PASSWORD, true), "a password field hides the cloud half");
        check(!CloudClipboardPanelPolicy.cloudAllowed(
            text | InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD, true), "a visible password field hides the cloud half");
        check(!CloudClipboardPanelPolicy.cloudAllowed(
            text | InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD, true), "a web password field hides the cloud half");
        check(!CloudClipboardPanelPolicy.cloudAllowed(
            InputType.TYPE_CLASS_NUMBER | InputType.TYPE_NUMBER_VARIATION_PASSWORD, true), "a PIN field hides the cloud half");
        check(!CloudClipboardPanelPolicy.cloudAllowed(text, false), "a private field (no personalised learning) hides the cloud half");

        check(CloudClipboardPanelPolicy.panelAvailable(true, false), "local history alone opens the panel");
        check(CloudClipboardPanelPolicy.panelAvailable(false, true), "the cloud half alone opens the panel");
        check(!CloudClipboardPanelPolicy.panelAvailable(false, false), "nothing to show keeps the panel closed");

        check(CloudClipboardPanelPolicy.initialTab(Tab.CLOUD, false) == Tab.LOCAL, "a sensitive field never opens on the cloud half");
        check(CloudClipboardPanelPolicy.initialTab(Tab.LOCAL, true) == Tab.LOCAL, "with local history off the panel still opens on the local half, where it can be turned on");
        check(CloudClipboardPanelPolicy.initialTab(Tab.CLOUD, true) == Tab.CLOUD, "the last chosen cloud half is kept");
        check(CloudClipboardPanelPolicy.initialTab(null, true) == Tab.LOCAL, "the panel opens on local history by default");

        check(CloudClipboardPanelPolicy.loaded(false, 3) == Status.DISABLED, "a disabled account shows no items");
        check(CloudClipboardPanelPolicy.loaded(true, 0) == Status.EMPTY, "an enabled empty list is empty");
        check(CloudClipboardPanelPolicy.loaded(true, 2) == Status.READY, "an enabled list with items is ready");
        check(CloudClipboardPanelPolicy.failed(401) == Status.SIGNED_OUT, "401 means signed out");
        check(CloudClipboardPanelPolicy.failed(403) == Status.SIGNED_OUT, "403 means signed out");
        check(CloudClipboardPanelPolicy.failed(500) == Status.FAILED, "a server error is retryable");
        check(CloudClipboardPanelPolicy.failed(0) == Status.FAILED, "a transport failure is retryable");
        // 端到端加密：本机待批准时只显示一句提示，不列条目也不上传；驱动的错误码优先于状态码。
        check(CloudClipboardPanelPolicy.loaded(true, true, 0) == Status.PENDING, "a device awaiting approval is pending");
        check(CloudClipboardPanelPolicy.loaded(false, true, 0) == Status.DISABLED, "a disabled account wins over pending");
        check(CloudClipboardPanelPolicy.loaded(true, false, 2) == Status.READY, "an approved device lists its items");
        check(CloudClipboardPanelPolicy.failed(0, "clipboard_device_pending") == Status.PENDING,
            "an upload refused while awaiting approval reads as pending, not a failure");
        check(CloudClipboardPanelPolicy.failed(403, "clipboard_sync_disabled") == Status.DISABLED,
            "a 403 from a disabled cloud clipboard is not a sign-out");
        check(CloudClipboardPanelPolicy.failed(403, "forbidden") == Status.SIGNED_OUT, "any other 403 still means signed out");
        check(CloudClipboardPanelPolicy.failed(409, "clipboard_key_conflict") == Status.FAILED, "a key conflict is retryable");
        check(!CloudClipboardPanelPolicy.showsItems(Status.PENDING), "a pending device lists nothing");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.PENDING, "synthetic note"), "a pending device uploads nothing");
        check(CloudClipboardPanelPolicy.cloudCount(Status.PENDING, 0) == null, "a pending device shows no count");
        check(CloudClipboardPanelPolicy.message(Status.PENDING, 0).equals(CloudClipboardPanelPolicy.PENDING_MESSAGE)
            && !CloudClipboardPanelPolicy.PENDING_MESSAGE.contains("\n"), "the pending hint is one line");

        check(CloudClipboardPanelPolicy.accepts(4, 4), "a result for the same field is drawn");
        check(!CloudClipboardPanelPolicy.accepts(4, 5), "a result for a previous field is discarded");
        check(CloudClipboardPanelPolicy.acceptsAccount("account-a", "account-a"),
            "a result for the current account is drawn");
        check(!CloudClipboardPanelPolicy.acceptsAccount("account-a", "account-b"),
            "a result from a previous account is discarded");
        check(!CloudClipboardPanelPolicy.acceptsAccount("account-a", ""),
            "a result is discarded after sign-out");
        check(CloudClipboardPanelPolicy.acceptsBinding("account-a", 7, "account-a", 7),
            "a result for the same account binding is drawn");
        check(!CloudClipboardPanelPolicy.acceptsBinding("account-a", 7, "account-a", 8),
            "a result from a previous sign-in lineage is discarded");
        check(!CloudClipboardPanelPolicy.acceptsBinding("account-a", 7, "account-b", 7),
            "a result from another account is discarded even with the same generation");
        check(!CloudClipboardPanelPolicy.acceptsBinding("", 7, "", 7),
            "an empty account binding is never accepted");
        check(!CloudClipboardPanelPolicy.acceptsBinding("account-a", -1, "account-a", -1),
            "an unavailable binding generation is never accepted");
        check(CloudClipboardPanelPolicy.canUseCloudRows(true, Status.READY,
                "account-a", 7, "account-a", 7),
            "cloud rows remain usable only for the loaded account binding");
        check(!CloudClipboardPanelPolicy.canUseCloudRows(true, Status.READY,
                "account-a", 7, "account-b", 8),
            "stale cloud rows are unusable after an account change");
        check(!CloudClipboardPanelPolicy.canUseCloudRows(true, Status.EMPTY,
                "account-a", 7, "account-a", 7),
            "an empty cloud page has no rows to use");
        check(CloudClipboardPanelPolicy.acceptsUploadResult(4, 4),
            "an upload result for the same panel may show its status");
        check(!CloudClipboardPanelPolicy.acceptsUploadResult(4, 5),
            "an upload result for a closed panel stays silent");
        check(CloudClipboardPanelPolicy.showsItems(Status.READY), "a ready list draws entries");
        for (Status status : Status.values()) {
            if (status != Status.READY) check(!CloudClipboardPanelPolicy.showsItems(status), status + " draws only a status line");
        }

        check(CloudClipboardPanelPolicy.canUpload(true, Status.READY, "synthetic note"), "a signed-in enabled account can receive an upload");
        check(CloudClipboardPanelPolicy.canUpload(true, Status.EMPTY, "synthetic note"), "an empty enabled list can receive an upload");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.SIGNED_OUT, "synthetic note"), "signed out cannot upload");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.DISABLED, "synthetic note"), "a disabled cloud clipboard cannot upload");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.LOADING, "synthetic note"), "an unanswered fetch cannot upload");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.FAILED, "synthetic note"), "a failed fetch cannot upload");
        check(!CloudClipboardPanelPolicy.canUpload(false, Status.READY, "synthetic note"), "a sensitive field cannot upload");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.READY, "x".repeat(4_001)), "text over the service limit cannot upload");
        check(!CloudClipboardPanelPolicy.canUpload(true, Status.READY, "  "), "blank text cannot upload");
        check(!CloudClipboardPanelPolicy.canMutate(false, false), "a failed reload cannot mutate stale cloud rows");
        check(!CloudClipboardPanelPolicy.canMutate(true, true), "a busy cloud action cannot overlap another mutation");
        check(CloudClipboardPanelPolicy.canMutate(true, false), "a loaded idle cloud page may mutate");

        check(CloudClipboardPanelPolicy.message(Status.SIGNED_OUT, 0).equals("登录水杉账号后可在设备间同步剪贴板"), "the shared signed-out wording is used");
        check(CloudClipboardPanelPolicy.message(Status.DISABLED, 0).equals("云剪贴板未开启"), "the shared disabled wording is used");
        check(CloudClipboardPanelPolicy.message(Status.READY, 3).startsWith("3 条"), "the ready line counts entries");
        check(CloudClipboardPanelPolicy.TAB_CLOUD.equals("云端"), "the cloud half uses the shared label");
        check(CloudClipboardPanelPolicy.UPLOAD_ACTION.equals("发到云剪贴板"), "the upload action uses the shared label");

        // #5905：顶行居中的「已存条数/上限」。
        check(CloudClipboardPanelPolicy.limit(Tab.LOCAL) == ClipboardHistoryPolicy.LIMIT, "the local limit is the shared store's");
        check(CloudClipboardPanelPolicy.limit(Tab.CLOUD) == CloudClipboardApi.MAX_ITEMS, "the cloud limit is the service's");
        check("0/200".equals(CloudClipboardPanelPolicy.countLabel(Tab.LOCAL, 0)), "an empty local history reads 0/200");
        check("10/200".equals(CloudClipboardPanelPolicy.countLabel(Tab.LOCAL, 10)), "ten local entries read 10/200");
        check("200/200".equals(CloudClipboardPanelPolicy.countLabel(Tab.LOCAL, 200)), "a full local history reads 200/200");
        check(CloudClipboardPanelPolicy.countLabel(Tab.LOCAL, null).isEmpty(), "an unreadable or disabled local history shows no count");
        check("本机已存 10 条，最多 200 条".equals(CloudClipboardPanelPolicy.countDescription(Tab.LOCAL, 10)),
            "the screen reader hears what the count means");
        check(CloudClipboardPanelPolicy.countDescription(Tab.LOCAL, null).isEmpty(), "no count, nothing to announce");
        Integer cloudReady = CloudClipboardPanelPolicy.cloudCount(Status.READY, 3);
        check(cloudReady != null && cloudReady == 3, "a ready cloud list is counted");
        check("3/50".equals(CloudClipboardPanelPolicy.countLabel(Tab.CLOUD, cloudReady)), "the cloud count reads 3/50");
        check("云端已存 3 条，最多 50 条".equals(CloudClipboardPanelPolicy.countDescription(Tab.CLOUD, cloudReady)),
            "the cloud count names its half");
        Integer cloudEmpty = CloudClipboardPanelPolicy.cloudCount(Status.EMPTY, 0);
        check(cloudEmpty != null && cloudEmpty == 0, "an enabled empty cloud list counts zero");
        for (Status status : new Status[] {Status.LOADING, Status.SIGNED_OUT, Status.DISABLED, Status.FAILED}) {
            check(CloudClipboardPanelPolicy.cloudCount(status, 0) == null, status + " shows no cloud count");
            check(CloudClipboardPanelPolicy.countLabel(Tab.CLOUD, CloudClipboardPanelPolicy.cloudCount(status, 0)).isEmpty(),
                status + " leaves the header gap empty");
        }
        for (Status status : Status.values()) {
            check(!CloudClipboardPanelPolicy.message(status, 1).isEmpty(), status + " has a message");
        }
        boolean rejected = false;
        try {
            CloudClipboardPanelPolicy.message(null, 0);
        } catch (IllegalArgumentException expected) {
            rejected = true;
        }
        check(rejected, "a missing status is rejected");
        System.out.println("Android cloud clipboard panel: field, tab, status and upload policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
