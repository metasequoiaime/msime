#include "tests/includes/test_framework.h"

#include "src/tsf/TextEditor.h"
#include "src/tsf/TsfTextServices.h"

namespace
{
class EditorFixture
{
  public:
    EditorFixture()
    {
        REQUIRE(SUCCEEDED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED)));
        REQUIRE(InitializeTsfTextServices(GetModuleHandleW(nullptr)));
        editor.SetWnd(nullptr);
        REQUIRE(editor.InitTSF());
        REQUIRE(editor.InsertAtSelection(L"ABCDE"));
    }

    ~EditorFixture()
    {
        editor.UninitTSF();
        UninitializeTsfTextServices();
        CoUninitialize();
    }

    CTextEditor editor;
};
} // namespace

TEST_CASE(text_selection_right_collapses_to_the_selected_range_end)
{
    EditorFixture fixture;
    fixture.editor.MoveSelection(1, 3);
    fixture.editor.MoveSelectionNext();

    REQUIRE(fixture.editor.GetSelectionStart() == 3);
    REQUIRE(fixture.editor.GetSelectionEnd() == 3);
}

TEST_CASE(text_selection_left_collapses_to_the_selected_range_start)
{
    EditorFixture fixture;
    fixture.editor.MoveSelection(1, 3);
    fixture.editor.MoveSelectionPrev();

    REQUIRE(fixture.editor.GetSelectionStart() == 1);
    REQUIRE(fixture.editor.GetSelectionEnd() == 1);
}

TEST_CASE(text_selection_arrows_move_an_unselected_caret)
{
    EditorFixture fixture;
    fixture.editor.MoveSelection(2, 2);
    fixture.editor.MoveSelectionNext();
    REQUIRE(fixture.editor.GetSelectionStart() == 3);
    REQUIRE(fixture.editor.GetSelectionEnd() == 3);
    fixture.editor.MoveSelectionPrev();
    REQUIRE(fixture.editor.GetSelectionStart() == 2);
    REQUIRE(fixture.editor.GetSelectionEnd() == 2);
}

TEST_CASE(text_selection_arrows_stay_at_text_boundaries)
{
    EditorFixture fixture;
    fixture.editor.MoveSelection(0, 0);
    fixture.editor.MoveSelectionPrev();
    REQUIRE(fixture.editor.GetSelectionStart() == 0);
    REQUIRE(fixture.editor.GetSelectionEnd() == 0);
    fixture.editor.MoveSelection(5, 5);
    fixture.editor.MoveSelectionNext();
    REQUIRE(fixture.editor.GetSelectionStart() == 5);
    REQUIRE(fixture.editor.GetSelectionEnd() == 5);
}

TEST_CASE(text_editor_rejects_null_insert_text)
{
    CTextEditor editor;
    REQUIRE(!editor.InsertAtSelection(nullptr));
    REQUIRE(editor.GetTextLength() == 0);
}
