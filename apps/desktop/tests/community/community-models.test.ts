import { expect, expectTypeOf, test } from "vitest";
import type * as Public from "@msime/ui";
import * as Skin from "../../../../packages/ui/src/community/community-skin-model";
import * as Candidate from "../../../../packages/ui/src/community/community-candidate-skin-model";
import * as Plugin from "../../../../packages/ui/src/community/community-plugin-model";
import * as Category from "../../../../packages/ui/src/community/community-skin-category-model";
import * as Report from "../../../../packages/ui/src/community/community-report-model";
import type * as OldSkin from "../../../../packages/ui/src/community/community-skins";
import * as OldCandidate from "../../../../packages/ui/src/community/community-candidate-skins";
import * as OldPlugin from "../../../../packages/ui/src/community/community-plugins";
import * as OldCategory from "../../../../packages/ui/src/community/community-skin-category";
import * as OldReport from "../../../../packages/ui/src/community/community-report";
import { useCommunitySkinCategoryFilter } from "../../../../packages/ui/src/community/use-community-skin-category-filter";

test("社区模型保留包入口与页面出口的全部类型契约", () => {
  type SkinTypes = [
    Skin.CommunitySkin,
    Skin.CommunitySkinClient,
    Skin.CommunitySkinDownload,
    Skin.CommunitySkinPage,
    Skin.CommunitySkinTrial,
  ];
  expectTypeOf<SkinTypes>().toEqualTypeOf<
    [
      OldSkin.CommunitySkin,
      OldSkin.CommunitySkinClient,
      OldSkin.CommunitySkinDownload,
      OldSkin.CommunitySkinPage,
      OldSkin.CommunitySkinTrial,
    ]
  >();
  expectTypeOf<SkinTypes>().toEqualTypeOf<
    [
      Public.CommunitySkin,
      Public.CommunitySkinClient,
      Public.CommunitySkinDownload,
      Public.CommunitySkinPage,
      Public.CommunitySkinTrial,
    ]
  >();
  type CandidateTypes = [
    Candidate.CandidateSkinCategory,
    Candidate.CandidateSkinCommunityClient,
    Candidate.CandidateSkinPackPreview,
    Candidate.CandidateSkinSyncReport,
    Candidate.CandidateSkinSyncSkip,
    Candidate.CandidateSkinVisibility,
    Candidate.CommunityCandidateSkin,
    Candidate.CommunityCandidateSkinLicense,
    Candidate.CommunityCandidateSkinPage,
  ];
  expectTypeOf<CandidateTypes>().toEqualTypeOf<
    [
      OldCandidate.CandidateSkinCategory,
      OldCandidate.CandidateSkinCommunityClient,
      OldCandidate.CandidateSkinPackPreview,
      OldCandidate.CandidateSkinSyncReport,
      OldCandidate.CandidateSkinSyncSkip,
      OldCandidate.CandidateSkinVisibility,
      OldCandidate.CommunityCandidateSkin,
      OldCandidate.CommunityCandidateSkinLicense,
      OldCandidate.CommunityCandidateSkinPage,
    ]
  >();
  expectTypeOf<CandidateTypes>().toEqualTypeOf<
    [
      Public.CandidateSkinCategory,
      Public.CandidateSkinCommunityClient,
      Public.CandidateSkinPackPreview,
      Public.CandidateSkinSyncReport,
      Public.CandidateSkinSyncSkip,
      Public.CandidateSkinVisibility,
      Public.CommunityCandidateSkin,
      Public.CommunityCandidateSkinLicense,
      Public.CommunityCandidateSkinPage,
    ]
  >();
  type PluginTypes = [
    Plugin.CommunityPlugin,
    Plugin.CommunityPluginClient,
    Plugin.CommunityPluginKind,
    Plugin.CommunityPluginPackPreview,
    Plugin.CommunityPluginPage,
  ];
  expectTypeOf<PluginTypes>().toEqualTypeOf<
    [
      OldPlugin.CommunityPlugin,
      OldPlugin.CommunityPluginClient,
      OldPlugin.CommunityPluginKind,
      OldPlugin.CommunityPluginPackPreview,
      OldPlugin.CommunityPluginPage,
    ]
  >();
  expectTypeOf<PluginTypes>().toEqualTypeOf<
    [
      Public.CommunityPlugin,
      Public.CommunityPluginClient,
      Public.CommunityPluginKind,
      Public.CommunityPluginPackPreview,
      Public.CommunityPluginPage,
    ]
  >();
  expectTypeOf<
    [Report.CommunityModeration, Report.CommunityReportKind, Report.CommunityReportReason]
  >().toEqualTypeOf<
    [OldReport.CommunityModeration, OldReport.CommunityReportKind, OldReport.CommunityReportReason]
  >();
  expectTypeOf<
    [Report.CommunityModeration, Report.CommunityReportKind, Report.CommunityReportReason]
  >().toEqualTypeOf<
    [Public.CommunityModeration, Public.CommunityReportKind, Public.CommunityReportReason]
  >();
  expectTypeOf<Category.CommunitySkinCategory>().toEqualTypeOf<OldCategory.CommunitySkinCategory>();
});

test("社区常量与分类 hook 的兼容出口保留同一对象", () => {
  expect(Candidate.candidateSkinCategories).toBe(Category.communitySkinCategories);
  expect(Candidate.candidateSkinCategoryLabels).toBe(Category.communitySkinCategoryLabels);
  expect(OldCandidate.candidateSkinCategories).toBe(Candidate.candidateSkinCategories);
  expect(OldCandidate.candidateSkinCategoryLabels).toBe(Candidate.candidateSkinCategoryLabels);
  expect(OldCategory.communitySkinCategories).toBe(Category.communitySkinCategories);
  expect(OldCategory.communitySkinCategoryLabels).toBe(Category.communitySkinCategoryLabels);
  expect(OldCategory.communitySkinCategoryLabel).toBe(Category.communitySkinCategoryLabel);
  expect(OldCategory.useCommunitySkinCategoryFilter).toBe(useCommunitySkinCategoryFilter);
  expect(OldPlugin.communityPluginKinds).toBe(Plugin.communityPluginKinds);
  expect(OldReport.communityReportReasons).toBe(Report.communityReportReasons);
  expect(OldReport.communityReportDetailLimit).toBe(Report.communityReportDetailLimit);
  expect(OldReport.communityReportedNotice).toBe(Report.communityReportedNotice);
});
