package app.msime.android;

import app.msime.android.core.InputViewValuePolicy;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 候选行这一刻要为提示多留几行：辅助码、五笔编码这类注释画在候选字下面另起一行，候选条没开释义时只有一行高，两行的候选被居中裁掉下半截。
 *
 * <p>只在这一页候选里确实有提示时才加高，没有提示时候选行保持紧凑（#11435 的 32 dp）。纠错行不在其列：它只在候选字后面画 *，不另起一行（CandidateCorrectionMarkPolicy）。代价是出现提示的那一刻窗口变高；横向滑动候选不改变高度，因为这里看的是整页候选，不是屏幕上露出的那几个。
 */
final class CandidateHintRows {
    private CandidateHintRows() { }

    /** 当前这一页候选里提示最多的那一个占几行（不含候选字本身），没有提示时为 0。 */
    static int rows(MSIMEInputService service) {
        JSONObject view = service.view;
        JSONArray candidates = view == null ? null : view.optJSONArray("candidates");
        if (candidates == null) return 0;
        String typed = InputViewValuePolicy.textOr(view, "preedit", "");
        int rows = 0;
        for (int index = 0; index < candidates.length(); index++) {
            JSONObject candidate = candidates.optJSONObject(index);
            if (candidate == null
                    || !AiPendingCandidatePolicy.isVisible(InputViewValuePolicy.integer(candidate, "source", -1)))
                continue;
            String annotation = service.candidatePronunciationAnnotation(
                service.candidateAnnotation(candidate, typed), candidate);
            rows = Math.max(rows, MSIMEInputService.candidateLabelLines(annotation) - 1);
        }
        return rows;
    }
}
