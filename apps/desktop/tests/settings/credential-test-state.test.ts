import { expectTypeOf, test } from "vitest";
import type { CredentialTestState } from "@msime/ui";
import type { CredentialTestState as SectionCredentialTestState } from "../../../../packages/ui/src/settings/credential-test-section";
import type { CredentialTestState as ModelCredentialTestState } from "../../../../packages/ui/src/settings/credential-test-state";

test("凭据测试状态从旧组件路径和包入口保持同一公共契约", () => {
  expectTypeOf<CredentialTestState>().toEqualTypeOf<ModelCredentialTestState>();
  expectTypeOf<CredentialTestState>().toEqualTypeOf<SectionCredentialTestState>();
});
