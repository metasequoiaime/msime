import { expect, expectTypeOf, test } from "vitest";
import {
  aiCredentialEditorModel,
  aiCredentialSaveConfig,
  aiProviderCredentialTestConfig,
  aiServiceCredentialTestConfig,
  aiCredentialTestDisabled,
  aiServiceCredentialTestDisabled,
  type AiCredentialEditorModelOptions,
  type AiCredentialStored,
  type AiCredentialSaveInput,
  type AiCredentialPreferences,
  type AiCredentialTestPreferences,
  type AiAssistantPreferences,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
} from "@msime/ui";
import { aiCredentialSaveConfig as leafSave } from "../../../../packages/ui/src/settings/ai-credential-save-config";
import {
  aiProviderCredentialTestConfig as leafProviderTest,
  aiServiceCredentialTestConfig as leafServiceTest,
  aiCredentialTestDisabled as leafTestDisabled,
  aiServiceCredentialTestDisabled as leafServiceTestDisabled,
} from "../../../../packages/ui/src/settings/ai-credential-test-config";
import type { AiCredentialStored as SectionStored } from "../../../../packages/ui/src/settings/ai-credential-section";
import type { AiCredentialStored as ModelStored } from "../../../../packages/ui/src/settings/ai-credential-model";

const base: AiCredentialEditorModelOptions = {
  endpoint: "https://ai.example.test/v1",
  model: "synthetic-model",
  origin: "https://ai.example.test:443",
  token: "",
  invalid: false,
  busy: false,
};
const stored = { endpoint: base.endpoint, model: base.model };

test("未保存时说明配置目录归属，输入可编辑且清除不展示", () => {
  expect(typeof aiCredentialEditorModel).toBe("function");
  const model = aiCredentialEditorModel(base);
  // 状态常驻，凭据写在哪里是解释，进「?」。
  expect(model.status).toBe("尚未保存");
  expect(model.description).toContain("用户配置目录");
  expect(model.matchesStored).toBe(false);
  expect(model.hasStoredCredential).toBe(false);
  expect(model.tokenDisabled).toBe(false);
  expect(model.saveDisabled).toBe(true);
  expect(model.clearDisabled).toBe(false);
});

test("已存绑定按原始接口和模型精确匹配，不自动裁剪", () => {
  expect(typeof aiCredentialEditorModel).toBe("function");
  const model = aiCredentialEditorModel({ ...base, stored });
  expect(model.matchesStored).toBe(true);
  expect(model.status).toContain("留空则保留原凭据");
  for (const patch of [{ endpoint: base.endpoint + "/" }, { model: ` ${base.model} ` }]) {
    const mismatch = aiCredentialEditorModel({ ...base, stored, ...patch });
    expect(mismatch.matchesStored).toBe(false);
    expect(mismatch.status).toContain(`${stored.endpoint}（${stored.model}）`);
    expect(mismatch.status).toContain("不一致");
  }
});

test("损坏状态说明优先于已存绑定匹配或未保存状态", () => {
  expect(typeof aiCredentialEditorModel).toBe("function");
  for (const credential of [undefined, stored, { ...stored, model: "previous-model" }]) {
    expect(
      aiCredentialEditorModel({ ...base, stored: credential, invalid: true }).status,
    ).toContain("ai-provider.json 无效");
  }
});

test("保存要求有效来源和非空模型，新凭据还需非空密钥", () => {
  expect(typeof aiCredentialEditorModel).toBe("function");
  expect(aiCredentialEditorModel({ ...base, token: "synthetic-token" }).saveDisabled).toBe(false);
  expect(aiCredentialEditorModel({ ...base, token: " \t" }).saveDisabled).toBe(true);
  expect(aiCredentialEditorModel({ ...base, stored, token: " \t" }).saveDisabled).toBe(false);
  for (const patch of [{ origin: null }, { origin: "" }, { model: " " }, { busy: true }]) {
    expect(
      aiCredentialEditorModel({ ...base, stored, token: "synthetic-token", ...patch }).saveDisabled,
    ).toBe(true);
  }
});

test("忙碌时禁用保存清除，密钥输入只随来源有效性变化", () => {
  expect(typeof aiCredentialEditorModel).toBe("function");
  const busy = aiCredentialEditorModel({ ...base, stored, busy: true });
  expect(busy.clearDisabled).toBe(true);
  expect(busy.tokenDisabled).toBe(false);
  expect(busy.hasStoredCredential).toBe(true);
  const noOrigin = aiCredentialEditorModel({ ...base, stored, origin: null });
  expect(noOrigin.tokenDisabled).toBe(true);
  expect(noOrigin.clearDisabled).toBe(false);
});

test("保存与配置测试公共出口复用原函数，保存保留原文并省略空白密钥", () => {
  expect(typeof aiCredentialSaveConfig).toBe("function");
  expect(aiCredentialSaveConfig).toBe(leafSave);
  expect(aiProviderCredentialTestConfig).toBe(leafProviderTest);
  expect(aiServiceCredentialTestConfig).toBe(leafServiceTest);
  expect(aiCredentialTestDisabled).toBe(leafTestDisabled);
  expect(aiServiceCredentialTestDisabled).toBe(leafServiceTestDisabled);
  const ai = { provider: "openai", endpoint: base.endpoint, model: base.model };
  expect(aiCredentialSaveConfig(ai, " synthetic-token ")).toEqual({
    ...ai,
    token: " synthetic-token ",
  });
  expect(aiCredentialSaveConfig(ai, " \t")).toEqual(ai);
});

test("已存元数据、保存参数和偏好投影在宿主及旧路径保持兼容", () => {
  expectTypeOf<AiCredentialStored>().toEqualTypeOf<SectionStored>();
  expectTypeOf<AiCredentialStored>().toEqualTypeOf<ModelStored>();
  expectTypeOf<AiCredentialStored>().toEqualTypeOf<
    Pick<ProviderCredentialStatus["ai"][number], "endpoint" | "model">
  >();
  expectTypeOf<AiCredentialSaveInput>().toEqualTypeOf<
    Parameters<ProviderCredentialClient["saveAi"]>[0]
  >();
  expectTypeOf<ReturnType<typeof aiCredentialSaveConfig>>().toEqualTypeOf<AiCredentialSaveInput>();
  expectTypeOf<AiCredentialPreferences>().toEqualTypeOf<
    Pick<AiAssistantPreferences, "provider" | "endpoint" | "model">
  >();
  expectTypeOf<AiCredentialTestPreferences>().toEqualTypeOf<
    Pick<AiAssistantPreferences, "enabled" | "model">
  >();
});
