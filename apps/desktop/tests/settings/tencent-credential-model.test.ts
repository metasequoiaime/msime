import { expect, expectTypeOf, test } from "vitest";
import {
  tencentCredentialEditorModel,
  tencentCredentialSavePayload,
  type TencentCredentialInput,
  type TencentCredentialStatus,
  type TencentCredentialSaveInput,
  type TencentCredentialSaveOptions,
  type LinuxTencentCredentialInput,
  type LinuxTencentCredentialStatus,
  type ProviderCredentialClient,
  type ProviderCredentialStatus,
} from "@msime/ui";
import type { TencentCredentialInput as HookInput } from "../../../../packages/ui/src/settings/use-provider-credentials";
import type {
  LinuxTencentCredentialInput as SectionInput,
  LinuxTencentCredentialStatus as SectionStatus,
} from "../../../../packages/ui/src/settings/linux-tencent-credentials-section";
import type { TencentCredentialInput as ModelInput } from "../../../../packages/ui/src/settings/tencent-credential-model";

const emptyInput = { secretId: "", secretKey: "" };
const stored: TencentCredentialStatus = {
  tencent: { region: "ap-shanghai" },
  tencentInvalid: false,
};

test("地域按输入、已存凭据和默认值回退，显式空串覆盖旧值", () => {
  expect(typeof tencentCredentialEditorModel).toBe("function");
  expect(tencentCredentialEditorModel({ input: emptyInput, busy: false }).region).toBe(
    "ap-guangzhou",
  );
  expect(
    tencentCredentialEditorModel({ input: emptyInput, status: stored, busy: false }).region,
  ).toBe("ap-shanghai");
  expect(
    tencentCredentialEditorModel({
      input: { ...emptyInput, region: "" },
      status: stored,
      busy: false,
    }).region,
  ).toBe("");
  expect(
    tencentCredentialEditorModel({
      input: { ...emptyInput, region: "ap-hongkong" },
      status: stored,
      busy: false,
    }).region,
  ).toBe("ap-hongkong");
});

test("凭据损坏提示优先于已存状态，没有凭据时说明用户目录归属", () => {
  expect(typeof tencentCredentialEditorModel).toBe("function");
  // 还没保存时没有状态可显示，凭据写在哪里是解释，进「在线翻译服务」的「?」。
  expect(tencentCredentialEditorModel({ input: emptyInput, busy: false }).status).toBeNull();
  expect(tencentCredentialEditorModel({ input: emptyInput, busy: false }).description).toContain(
    "用户配置目录",
  );
  expect(
    tencentCredentialEditorModel({ input: emptyInput, status: stored, busy: false }).status,
  ).toContain("留空则保留原值");
  expect(
    tencentCredentialEditorModel({
      input: emptyInput,
      status: { ...stored, tencentInvalid: true },
      busy: false,
    }).status,
  ).toContain("tencent-provider.json 无效");
});

test("新凭据要求两个密钥，已有凭据允许省略或只更新其中一个", () => {
  expect(typeof tencentCredentialEditorModel).toBe("function");
  for (const input of [
    emptyInput,
    { secretId: "synthetic-id", secretKey: " " },
    { secretId: " ", secretKey: "synthetic-key" },
  ]) {
    expect(tencentCredentialEditorModel({ input, busy: false }).saveDisabled).toBe(true);
    expect(tencentCredentialEditorModel({ input, status: stored, busy: false }).saveDisabled).toBe(
      false,
    );
  }
  expect(
    tencentCredentialEditorModel({
      input: { secretId: "synthetic-id", secretKey: "synthetic-key" },
      busy: false,
    }).saveDisabled,
  ).toBe(false);
});

test("忙碌时禁用保存和清除，地域格式只决定默认展开", () => {
  expect(typeof tencentCredentialEditorModel).toBe("function");
  const input = { secretId: "synthetic-id", secretKey: "synthetic-key", region: "ap hongkong" };
  const model = tencentCredentialEditorModel({ input, status: stored, busy: false });
  expect(model.regionInvalid).toBe(true);
  expect(model.saveDisabled).toBe(false);
  expect(model.clearDisabled).toBe(false);
  expect(
    tencentCredentialEditorModel({ input: { ...input, region: "" }, busy: false }).regionInvalid,
  ).toBe(false);
  const busy = tencentCredentialEditorModel({ input, status: stored, busy: true });
  expect(busy.saveDisabled).toBe(true);
  expect(busy.clearDisabled).toBe(true);
});

test("保存只提交非空白密钥并保留原文，不从状态回填秘密", () => {
  expect(typeof tencentCredentialSavePayload).toBe("function");
  const options: TencentCredentialSaveOptions = {
    input: { secretId: " synthetic-id ", secretKey: " synthetic-key " },
    status: stored,
  };
  expect(tencentCredentialSavePayload(options)).toEqual({
    secretId: " synthetic-id ",
    secretKey: " synthetic-key ",
    region: "ap-shanghai",
  });
  expect(
    tencentCredentialSavePayload({
      input: { secretId: " ", secretKey: "\t", region: "" },
      status: stored,
    }),
  ).toEqual({ region: "" });
  expect(
    tencentCredentialSavePayload({ input: { secretId: "synthetic-id", secretKey: "" } }),
  ).toEqual({ secretId: "synthetic-id", region: "ap-guangzhou" });
});

test("包入口、旧深路径和宿主保存参数复用同一契约", () => {
  expectTypeOf<TencentCredentialInput>().toEqualTypeOf<ModelInput>();
  expectTypeOf<TencentCredentialInput>().toEqualTypeOf<HookInput>();
  expectTypeOf<TencentCredentialInput>().toEqualTypeOf<SectionInput>();
  expectTypeOf<TencentCredentialInput>().toEqualTypeOf<LinuxTencentCredentialInput>();
  expectTypeOf<TencentCredentialStatus>().toEqualTypeOf<SectionStatus>();
  expectTypeOf<TencentCredentialStatus>().toEqualTypeOf<LinuxTencentCredentialStatus>();
  expectTypeOf<TencentCredentialStatus>().toEqualTypeOf<
    Pick<ProviderCredentialStatus, "tencent" | "tencentInvalid">
  >();
  expectTypeOf<TencentCredentialSaveInput>().toEqualTypeOf<
    Parameters<ProviderCredentialClient["saveTencent"]>[0]
  >();
});
