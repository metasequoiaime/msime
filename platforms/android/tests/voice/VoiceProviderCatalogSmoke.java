package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/** 目录名称、顺序和流式地址来自持有快照，Java 只追加平台服务。 */
public final class VoiceProviderCatalogSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        var asr = new ArrayList<>(List.of(new VoiceProviderCatalog.Provider("future-asr", "合成识别")));
        var polish = new ArrayList<>(List.of(new VoiceProviderCatalog.Provider("future-polish", "合成润色")));
        var endpoints = List.of(
            new VoiceProviderCatalog.StreamEndpoint("first", "合成整句", "合成整句说明", "wss://example.invalid/first"),
            new VoiceProviderCatalog.StreamEndpoint("second", "合成双向", "合成双向说明", "wss://example.invalid/second"));
        var catalog = new VoiceProviderCatalog(asr, polish, endpoints, endpoints.get(1).endpoint());
        asr.clear();
        polish.clear();
        check(catalog.available(), "完整目录可供选择");
        check(catalog.asr().stream().map(VoiceProviderCatalog.Provider::id).toList()
            .equals(List.of("system", "local", "future-asr")), "平台服务在共享目录前，目录顺序保持不变");
        check(catalog.asrLabel(null).equals("系统识别") && catalog.asrLabel("").equals("系统识别"),
            "未设置时保留系统识别的显示语义");
        check(catalog.asrLabel("local").equals("本地模型")
            && catalog.asrLabel("future-asr").equals("合成识别"), "显示共享名称和平台名称");
        check(catalog.polishLabel("").equals("合成润色")
            && catalog.polishLabel("future-polish").equals("合成润色"), "默认润色名取目录首项");
        check(catalog.asrLabel("unknown").equals("unknown")
            && catalog.polishLabel("unknown").equals("unknown"), "未知配置仍显示原始服务 id");
        check(catalog.doubaoEndpointLabel("").equals("合成双向")
            && catalog.doubaoEndpointLabel("  wss://example.invalid/first  ").equals("合成整句"),
            "空地址显示共享默认接口，已有地址允许首尾空格");
        check(catalog.doubaoEndpointLabel("wss://example.invalid/custom").equals("自定义地址"),
            "目录外地址仍可自定义");
        check(catalog.selectedEndpoint("").equals(endpoints.get(1))
            && catalog.selectedEndpoint("wss://example.invalid/custom") == null, "选择状态使用共享地址匹配");
        try {
            catalog.asr().clear();
            throw new AssertionError("目录不能被界面修改");
        } catch (UnsupportedOperationException expected) {
            check(catalog.asr().size() == 3, "失败修改不破坏快照");
        }
        var unavailable = VoiceProviderCatalog.unavailable();
        check(!unavailable.available() && unavailable.asr().isEmpty() && unavailable.polish().isEmpty(),
            "失败时不提供第二份服务目录");
        check(unavailable.asrLabel("future-asr").equals("future-asr"), "读取失败仍显示当前配置");
        System.out.println("Android shared voice catalog: snapshot and presentation passed");
    }
}
