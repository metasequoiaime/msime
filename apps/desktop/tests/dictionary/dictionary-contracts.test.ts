import { expectTypeOf, test } from "vitest";
import type {
  DictionaryClient as PublicClient,
  DictionaryFailure as PublicFailure,
  DictionaryImportResult as PublicImportResult,
  DictionaryManifest as PublicManifest,
  WubiTextCodes as PublicTextCodes,
  WubiCodeLookup as PublicLookup,
  WubiCodeLookupClient as PublicLookupClient,
} from "@msime/ui";
import type {
  DictionaryClient,
  DictionaryFailure,
  DictionaryImportResult,
  DictionaryManifest,
  WubiTextCodes,
  WubiCodeLookup,
  WubiCodeLookupClient,
} from "../../../../packages/ui/src/dictionary/dictionary-contracts";
import type { DictionaryManifest as CardManifest } from "../../../../packages/ui/src/settings/dictionary-manifest-card";
import type { DictionaryFailureNotice } from "../../../../packages/ui/src/settings/dictionary-failures-notice";
import type { ImportSummary } from "../../../../packages/ui/src/dictionary/dictionary-messages";
import type {
  WubiTextCodes as SectionTextCodes,
  WubiCodeLookup as SectionLookup,
  WubiCodeLookupClient as SectionLookupClient,
} from "../../../../packages/ui/src/settings/wubi-code-lookup-section";

test("词库契约保持包入口和旧组件路径兼容", () => {
  expectTypeOf<DictionaryClient>().toEqualTypeOf<PublicClient>();
  expectTypeOf<DictionaryFailure>().toEqualTypeOf<PublicFailure>();
  expectTypeOf<DictionaryImportResult>().toEqualTypeOf<PublicImportResult>();
  expectTypeOf<DictionaryManifest>().toEqualTypeOf<PublicManifest>();
  expectTypeOf<DictionaryManifest>().toEqualTypeOf<CardManifest>();
  expectTypeOf<WubiTextCodes>().toEqualTypeOf<PublicTextCodes>();
  expectTypeOf<WubiTextCodes>().toEqualTypeOf<SectionTextCodes>();
  expectTypeOf<WubiCodeLookup>().toEqualTypeOf<PublicLookup>();
  expectTypeOf<WubiCodeLookup>().toEqualTypeOf<SectionLookup>();
  expectTypeOf<WubiCodeLookupClient>().toEqualTypeOf<PublicLookupClient>();
  expectTypeOf<WubiCodeLookupClient>().toEqualTypeOf<SectionLookupClient>();
});

test("展示模型复用词库响应并保留宽松导入摘要", () => {
  expectTypeOf<DictionaryFailureNotice>().toEqualTypeOf<DictionaryFailure>();
  expectTypeOf<{ [K in keyof ImportSummary]: ImportSummary[K] }>().toEqualTypeOf<{
    applied: number;
    failed?: number;
    truncated?: boolean;
    swapped?: boolean;
    first_failures?: { line: number; issue: string }[];
  }>();
  expectTypeOf<DictionaryImportResult>().toExtend<ImportSummary>();
  expectTypeOf<{ applied: number }>().toExtend<ImportSummary>();
  expectTypeOf<DictionaryImportResult["failed"]>().toEqualTypeOf<number>();
});

test("客户端保留最小能力及可变列表快照", () => {
  const minimal = {
    list: async () => ({ entries: [], has_more: false }),
    edit: async () => {},
  } satisfies DictionaryClient;
  expectTypeOf(minimal).toExtend<DictionaryClient>();
  type Page = Awaited<ReturnType<DictionaryClient["list"]>>;
  expectTypeOf<Page["failed_requests"]>().toEqualTypeOf<DictionaryFailure[] | undefined>();
  expectTypeOf<Page["snapshot_error"]>().toEqualTypeOf<string | null | undefined>();
  expectTypeOf<WubiCodeLookup["phrase"]>().toEqualTypeOf<WubiTextCodes | null>();
  expectTypeOf<WubiCodeLookup["characters"]>().toEqualTypeOf<WubiTextCodes[]>();
});
