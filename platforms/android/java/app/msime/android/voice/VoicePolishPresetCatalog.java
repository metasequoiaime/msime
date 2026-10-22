package app.msime.android;

// 由共享 Rust 润色目录导出；通过 export_polish_prompt_catalog 更新。
final class VoicePolishPresetCatalog {
    private VoicePolishPresetCatalog() {}

    static String[] ids() {
        return new String[] {"cleanup", "faithful", "zh2en", "casual"};
    }

    static String[] titles() {
        return new String[] {"精炼整理", "忠实校对", "中翻英", "口语整理"};
    }
}
