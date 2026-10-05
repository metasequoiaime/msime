package app.msime.android;

import android.graphics.Color;
import android.view.View;

/** 输入提示（扩展点）：候选区上方那行提示的显示、自动消失与清除；现在的行为就是原来的 showDiagnostic 与 diagnosticView 更新。 */
final class ImeDebugOverlay {
    private final MSIMEInputService s;

    ImeDebugOverlay(MSIMEInputService s) {
        this.s = s;
    }

    void showDiagnostic(String value) {
        s.diagnosticGeneration++;
        if (s.diagnosticDismissTask != null) {
            s.main.removeCallbacks(s.diagnosticDismissTask);
            s.diagnosticDismissTask = null;
        }
        s.diagnosticMessage = InputDiagnosticPolicy.normalize(value);
        if (s.diagnosticMessage.isEmpty()) return;
        long generation = s.diagnosticGeneration;
        s.diagnosticDismissTask = () -> {
            if (generation != s.diagnosticGeneration) return;
            s.diagnosticMessage = "";
            s.diagnosticDismissTask = null;
            s.render();
        };
        s.main.postDelayed(s.diagnosticDismissTask, InputDiagnosticPolicy.DISMISS_DELAY_MILLIS);
    }

    void clearDiagnostic() {
        s.diagnosticGeneration++;
        if (s.diagnosticDismissTask != null) {
            s.main.removeCallbacks(s.diagnosticDismissTask);
            s.diagnosticDismissTask = null;
        }
        s.diagnosticMessage = "";
    }

    void updateDiagnosticView(boolean hasDiagnostic) {
        if (s.diagnosticView == null) return;
        s.diagnosticView.setText(s.diagnosticMessage);
        s.diagnosticView.setContentDescription("提示：" + s.diagnosticMessage);
        s.diagnosticView.setTextColor(Color.parseColor(s.skin.accent()));
        s.diagnosticView.setVisibility(hasDiagnostic ? View.VISIBLE : View.GONE);
    }
}
