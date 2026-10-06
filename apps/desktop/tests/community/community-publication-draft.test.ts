// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { expect, test } from "vitest";
import { useCommunityPublicationDraft } from "../../../../packages/ui/src/community/use-community-publication-draft";

test("normalizes edited names and rotates the publication id for every content change", () => {
  const { result } = renderHook(() => useCommunityPublicationDraft());
  const initialPublicationId = result.current.publicationId;

  act(() => result.current.onNameChange("abcdefghijklmnopqrstuvwxyz0123456789"));
  expect(result.current.name).toBe("abcdefghijklmnopqrstuvwxyz012345");
  expect(result.current.publicationId).not.toBe(initialPublicationId);

  const namePublicationId = result.current.publicationId;
  act(() => result.current.onDescriptionChange("  合成说明  "));
  expect(result.current.description).toBe("  合成说明  ");
  expect(result.current.publicationId).not.toBe(namePublicationId);
});

test("can reset publication identity and update rights state", () => {
  const { result } = renderHook(() => useCommunityPublicationDraft());
  const initialPublicationId = result.current.publicationId;

  act(() => {
    result.current.onAgreedChange(true);
    result.current.resetPublication();
  });

  expect(result.current.agreed).toBe(true);
  expect(result.current.publicationId).not.toBe(initialPublicationId);
});
