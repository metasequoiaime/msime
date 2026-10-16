import { expectTypeOf, test } from "vitest";
import type { Preferences } from "@msime/ui";
import type {
  CustomTranslationCredentialPreferences,
  CustomTranslationPreferences,
  NiuTransCredentialPreferences,
  NiuTransPreferences,
  TencentTranslationCredentialPreferences,
  TencentTranslationPreferences,
} from "../../../../packages/ui/src/settings/translation-credential-preferences";

test("翻译服务偏好公共模型与设置模型保持一致", () => {
  expectTypeOf<CustomTranslationPreferences>().toEqualTypeOf<
    NonNullable<Preferences["custom_translation"]>
  >();
  expectTypeOf<TencentTranslationPreferences>().toEqualTypeOf<
    NonNullable<Preferences["tencent_tmt"]>
  >();
  expectTypeOf<NiuTransPreferences>().toEqualTypeOf<NonNullable<Preferences["niutrans"]>>();
});

test("翻译凭据测试使用各服务的最小公共投影", () => {
  expectTypeOf<NiuTransCredentialPreferences>().toEqualTypeOf<
    Pick<NiuTransPreferences, "app_id" | "apikey">
  >();
  expectTypeOf<TencentTranslationCredentialPreferences>().toEqualTypeOf<
    Pick<TencentTranslationPreferences, "secret_id" | "secret_key" | "region">
  >();
  expectTypeOf<CustomTranslationCredentialPreferences>().toEqualTypeOf<
    Pick<CustomTranslationPreferences, "endpoint" | "api_key">
  >();
});
