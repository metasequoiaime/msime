import { expect, test } from "vitest";
import {
  customTranslationCredentialTestConfig,
  customTranslationCredentialTestDisabled,
  niutransCredentialTestConfig,
  niutransCredentialTestDisabled,
  tencentTranslationCredentialTestConfig,
  tencentTranslationCredentialTestDisabled,
} from "../../../../packages/ui/src/settings/translation-credential-test-config";

test("builds NiuTrans credential test configuration", () => {
  expect(
    niutransCredentialTestConfig({ app_id: "synthetic-app", apikey: "synthetic-key" }),
  ).toEqual({
    app_id: "synthetic-app",
    apikey: "synthetic-key",
  });
});

test("builds Tencent translation credential test configuration", () => {
  expect(
    tencentTranslationCredentialTestConfig({
      secret_id: "synthetic-id",
      secret_key: "synthetic-key",
      region: "ap-test",
    }),
  ).toEqual({
    secret_id: "synthetic-id",
    secret_key: "synthetic-key",
    region: "ap-test",
  });
});

test("builds custom translation credential test configuration", () => {
  expect(
    customTranslationCredentialTestConfig({
      endpoint: "https://translation.example.test",
      api_key: "synthetic-key",
    }),
  ).toEqual({
    endpoint: "https://translation.example.test",
    api_key: "synthetic-key",
  });
});

test("shares translation credential test disabled conditions", () => {
  expect(niutransCredentialTestDisabled(true, { app_id: "app", apikey: "key" })).toBe(false);
  expect(niutransCredentialTestDisabled(false, { app_id: "app", apikey: "key" })).toBe(true);
  expect(niutransCredentialTestDisabled(true, { app_id: "", apikey: "key" })).toBe(true);

  expect(tencentTranslationCredentialTestDisabled(true, "")).toBe(false);
  expect(tencentTranslationCredentialTestDisabled(false, "")).toBe(true);
  expect(tencentTranslationCredentialTestDisabled(true, "凭据错误")).toBe(true);

  expect(customTranslationCredentialTestDisabled(true, "")).toBe(false);
  expect(customTranslationCredentialTestDisabled(false, "")).toBe(true);
  expect(customTranslationCredentialTestDisabled(true, "地址错误")).toBe(true);
});
