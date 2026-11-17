package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONException;
import org.json.JSONObject;

/** 异步结果只更新同一次组字的展开快照；打字和主动关闭仍由原有生命周期处理。 */
final class ImeCandidatePanelRefresh {
    private ImeCandidatePanelRefresh() {}

    static void accept(MSIMEInputService service, JSONObject next) {
        JSONObject previous = service.view;
        String editing = previous == null ? "" : InputViewValuePolicy.editingText(previous);
        boolean follows = service.candidatePanelOpen
            && service.candidateVersionIsCurrent(service.candidatePanelSnapshot)
            && next != null
            && CandidateGlossPolicy.strictOr(next.opt("session"), Long.MIN_VALUE) == service.session
            && !editing.isEmpty() && editing.equals(InputViewValuePolicy.editingText(next))
            && !MSIMEInputService.sameCandidateVersion(service.candidatePanelSnapshot, next);
        service.view = next;
        if (!follows) return;
        try {
            // 全列表可能再次换代，必须同步真实视图和每个候选的身份，不能只改 generation。
            service.candidatePanelSnapshot = service.currentCandidateSnapshot();
        } catch (JSONException | LinkageError error) {
            service.closeCandidatePanel();
        }
    }
}
