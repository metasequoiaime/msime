import { expectTypeOf, test } from "vitest";
import type {
  HostCapabilities as PublicCapabilities,
  HostPlatform as PublicPlatform,
  EditionInfo as PublicEdition,
  InputScheme as PublicScheme,
  ChineseScheme as PublicChineseScheme,
} from "@msime/ui";
import type {
  HostCapabilities,
  HostPlatform,
  EditionInfo,
} from "../../../../packages/ui/src/core/host-capabilities";
import type { InputScheme, ChineseScheme } from "../../../../packages/ui/src/core/input-scheme";

test("宿主能力与方案叶子契约保持原有公开导出", () => {
  expectTypeOf<HostCapabilities>().toEqualTypeOf<PublicCapabilities>();
  expectTypeOf<HostPlatform>().toEqualTypeOf<PublicPlatform>();
  expectTypeOf<EditionInfo>().toEqualTypeOf<PublicEdition>();
  expectTypeOf<InputScheme>().toEqualTypeOf<PublicScheme>();
  expectTypeOf<ChineseScheme>().toEqualTypeOf<PublicChineseScheme>();
});

test("宿主和版本引用同一方案模型并保留可变数组", () => {
  expectTypeOf<HostCapabilities["platform"]>().toEqualTypeOf<HostPlatform>();
  expectTypeOf<HostCapabilities["input_schemes"]>().toEqualTypeOf<InputScheme[]>();
  expectTypeOf<EditionInfo["input_schemes"]>().toEqualTypeOf<InputScheme[]>();
  expectTypeOf<EditionInfo["default_scheme"]>().toEqualTypeOf<InputScheme>();
  expectTypeOf<HostCapabilities["edition"]>().toEqualTypeOf<EditionInfo | undefined>();
});
