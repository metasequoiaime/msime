// @vitest-environment jsdom
import { cleanup, fireEvent, render, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { CandidateFontControls } from "../../../../packages/ui/src/candidate/candidate-font-controls";
import {
  candidateFamilyStyle,
  validCandidateFonts,
} from "../../../../packages/ui/src/candidate/candidate-font-family";
import { useResolvedCandidateFonts } from "../../../../packages/ui/src/candidate/resolved-candidate-fonts";
afterEach(cleanup);

test("Windows exposes an independent English face with a platform default", () => {
  const onChange = vi.fn();
  const view = render(<CandidateFontControls value={{}} onChange={onChange} windows />);
  const input = view.getByLabelText("英文字体") as HTMLInputElement;
  expect(input.value).toBe("Segoe UI");
  fireEvent.change(input, { target: { value: "Synthetic Latin" } });
  expect(onChange).toHaveBeenCalledWith({ candidate_english_font: "Synthetic Latin" });
  view.rerender(<CandidateFontControls value={{}} onChange={onChange} />);
  expect(view.queryByLabelText("英文字体")).toBeNull();
});

test("macOS exposes the English face while following the primary face by default", () => {
  const onChange = vi.fn();
  const view = render(
    <CandidateFontControls
      value={{ candidate_font_family: "PingFang SC" }}
      onChange={onChange}
      englishFont
    />,
  );
  const input = view.getByLabelText("英文字体") as HTMLInputElement;
  expect(input.value).toBe("PingFang SC");
  fireEvent.change(input, { target: { value: "Helvetica" } });
  expect(onChange).toHaveBeenCalledWith({ candidate_english_font: "Helvetica" });
});

test("English face leads the chain and resolution never overwrites stored choices", async () => {
  const preferences = {
    candidate_font_family: "Legacy",
    candidate_english_font: "Latin W03",
    candidate_fallback_fonts: ["CJK W04"],
  };
  const resolve = vi.fn().mockResolvedValue(["Latin", "Legacy", "CJK"]);
  const { result } = renderHook(() => useResolvedCandidateFonts(preferences, resolve));
  await waitFor(() => expect(result.current.candidate_english_font).toBe("Latin"));
  expect(resolve).toHaveBeenCalledWith(["Latin W03", "Legacy", "CJK W04"]);
  expect(result.current.candidate_font_family).toBe("Legacy");
  expect(preferences.candidate_english_font).toBe("Latin W03");
  expect(candidateFamilyStyle(result.current)).toEqual({
    "--appearance-font-family": '"Latin", "Legacy", "CJK", sans-serif',
  });
  expect(
    candidateFamilyStyle({ candidate_font_family: "Legacy", candidate_fallback_fonts: [] }),
  ).toEqual({ "--appearance-font-family": '"Legacy", sans-serif' });
});

test.each(["", "x".repeat(129), "bad\nname", "bad\u0085name"])(
  "invalid English font is rejected: %j",
  (name) => {
    expect(validCandidateFonts({ candidate_english_font: name })).toBe(false);
  },
);
