import { expect, test } from "vitest";
import {
  customTranslationCredentialTestConfig,
  niutransCredentialTestConfig,
  tencentTranslationCredentialTestConfig,
} from "../../../../packages/ui/src/settings/translation-credential-test-config";

test("builds NiuTrans credential test configuration", () => {
  expect(niutransCredentialTestConfig({ app_id: "synthetic-app", apikey: "synthetic-key" })).toEqual(
    {
      app_id: "synthetic-app",
      apikey: "synthetic-key",
    },
  );
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
