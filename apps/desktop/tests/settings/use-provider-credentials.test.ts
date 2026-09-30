// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test } from "vitest";
import { useProviderCredentials } from "@msime/ui";

test("merges successive Tencent credential input patches against the latest state", () => {
  const { result } = renderHook(() => useProviderCredentials({ client: {} }));

  act(() => {
    result.current.updateTencentCredentialInput({ secretId: "synthetic-id" });
    result.current.updateTencentCredentialInput({ secretKey: "synthetic-key" });
  });

  expect(result.current.tencentCredentialInput).toEqual({
    secretId: "synthetic-id",
    secretKey: "synthetic-key",
    region: undefined,
  });
});
