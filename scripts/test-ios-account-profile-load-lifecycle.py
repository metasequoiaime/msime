#!/usr/bin/env python3
"""Contract checks for page-owned account profile loading and nickname saves."""
from pathlib import Path
import re
import unittest


SOURCE = Path("platforms/ios/App/Sources/account/AccountProfileView.swift").read_text()
GENERATION = Path("platforms/ios/App/Sources/app/ViewRequestGeneration.swift").read_text()
TASK = Path("platforms/ios/App/Sources/app/ViewRequestTask.swift").read_text()


class AccountProfileLoadLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.profile = SOURCE.split("struct AccountProfileEditor", 1)[1].split(
            "private struct AccountNicknameEditor", 1
        )[0]
        self.nickname = SOURCE.split("private struct AccountNicknameEditor", 1)[1]

    def test_profile_load_is_owned_by_visible_page(self):
        self.assertIn("@State private var loadTask: Task<Void, Never>?", self.profile)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.profile)
        self.assertIn(".trackViewRequest(request: $request, pending: $operation, onAppear: {", self.profile)
        self.assertIn("startLoad()", self.profile)
        self.assertIn("ViewRequestTask.cancel(task: $loadTask)", self.profile)
        self.assertIn("ViewRequestTask.startLoad(request: $request, pending: $loadTask) { id in", self.profile)
        self.assertIn("try request.checkCurrent(id)", self.profile)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $operation)", self.profile)
        self.assertIn("catch is CancellationError", self.profile)
        self.assertIn("if request.wrappedValue.isCurrent(id) { pending.wrappedValue = nil }", TASK)

    def test_nickname_save_is_cancelled_when_editor_leaves(self):
        self.assertIn("@State private var operation: Task<Void, Never>?", self.nickname)
        self.assertIn("@State private var request = ViewRequestGeneration()", self.nickname)
        self.assertIn(".trackViewRequest(request: $request, pending: $operation, onAppear: {", self.nickname)
        self.assertNotIn("ViewRequestTask.disappear(request: $request, pending: $operation)", self.nickname)
        self.assertIn("ViewRequestTask.run(request: $request, busy: $saving, message: $message,", self.nickname)
        self.assertIn("pending: $operation", self.nickname)
        self.assertIn("isCancellation: AccountRequestPolicy.isCancellation", self.nickname)
        self.assertIn("try request.checkCurrent(id)", self.nickname)
        self.assertIn("try Task.checkCancellation()", GENERATION)
        self.assertIn("request.wrappedValue.disappear()", TASK)
        self.assertIn("pending.wrappedValue?.cancel()", TASK)
        self.assertIn("task.wrappedValue?.cancel()", TASK)


if __name__ == "__main__":
    unittest.main()
