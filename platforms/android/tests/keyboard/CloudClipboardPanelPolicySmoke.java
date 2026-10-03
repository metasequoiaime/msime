import android.text.InputType;
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

        check(CloudClipboardPanelPolicy.initialTab(Tab.CLOUD, true, false) == Tab.LOCAL, "a sensitive field never opens on the cloud half");
        check(CloudClipboardPanelPolicy.initialTab(Tab.LOCAL, false, true) == Tab.CLOUD, "with local history off the panel opens on the cloud half");
        check(CloudClipboardPanelPolicy.initialTab(Tab.CLOUD, true, true) == Tab.CLOUD, "the last chosen cloud half is kept");
        check(CloudClipboardPanelPolicy.initialTab(Tab.LOCAL, true, true) == Tab.LOCAL, "the last chosen local half is kept");
        check(CloudClipboardPanelPolicy.initialTab(null, true, true) == Tab.LOCAL, "the panel opens on local history by default");

        check(CloudClipboardPanelPolicy.loaded(false, 3) == Status.DISABLED, "a disabled account shows no items");
        check(CloudClipboardPanelPolicy.loaded(true, 0) == Status.EMPTY, "an enabled empty list is empty");
        check(CloudClipboardPanelPolicy.loaded(true, 2) == Status.READY, "an enabled list with items is ready");
        check(CloudClipboardPanelPolicy.failed(401) == Status.SIGNED_OUT, "401 means signed out");
        check(CloudClipboardPanelPolicy.failed(403) == Status.SIGNED_OUT, "403 means signed out");
        check(CloudClipboardPanelPolicy.failed(500) == Status.FAILED, "a server error is retryable");
        check(CloudClipboardPanelPolicy.failed(0) == Status.FAILED, "a transport failure is retryable");

        check(CloudClipboardPanelPolicy.accepts(4, 4), "a result for the same field is drawn");
        check(!CloudClipboardPanelPolicy.accepts(4, 5), "a result for a previous field is discarded");
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
