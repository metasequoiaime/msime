import { expectTypeOf, test } from "vitest";
import type {
  AiProviderCatalogEntry,
  AiProviderOption,
  ProviderDefaults,
  ProviderPreset,
} from "@msime/ui";
import type { ProviderDefaults as VoiceProviderDefaults } from "../../../../packages/ui/src/voice/voice-providers";
import type { ProviderPreset as SectionProviderPreset } from "../../../../packages/ui/src/settings/provider-preset-section";
import type { AiProviderOption as SectionAiProviderOption } from "../../../../packages/ui/src/settings/ai-settings-page-section";

test("服务商公共模型保留旧导出和结构类型的兼容性", () => {
  expectTypeOf<ProviderDefaults>().toEqualTypeOf<VoiceProviderDefaults>();
  expectTypeOf<ProviderPreset>().toEqualTypeOf<SectionProviderPreset>();
  expectTypeOf<AiProviderOption>().toEqualTypeOf<SectionAiProviderOption>();
  expectTypeOf<ProviderDefaults>().toExtend<Record<string, unknown>>();
  expectTypeOf<AiProviderCatalogEntry>().toExtend<Record<string, unknown>>();
  expectTypeOf<{ id: string; title: string }>().toExtend<AiProviderOption>();
  expectTypeOf<{}>().toExtend<ProviderPreset>();
});
