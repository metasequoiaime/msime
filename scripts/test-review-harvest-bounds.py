#!/usr/bin/env python3
"""Synthetic contract tests for the harvested-case review response limit."""
import importlib.util
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("review_harvested_cases", ROOT / "scripts/review-harvested-cases.py")
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
import os
os.environ.setdefault("TYPESAFE_API_KEY", "synthetic-test-key")
SPEC.loader.exec_module(MODULE)


class Response:
    def __init__(self, payload: bytes):
        self.payload = payload
        self.requested = []

    def read(self, size=-1):
        self.requested.append(size)
        return self.payload if size < 0 else self.payload[:size]


valid = Response(b'{"answers": {}}')
assert MODULE.read_response_json(valid) == {"answers": {}}
assert valid.requested == [MODULE.MAX_RESPONSE_BYTES + 1]

oversized = Response(b"x" * (MODULE.MAX_RESPONSE_BYTES + 1))
try:
    MODULE.read_response_json(oversized)
except ValueError as error:
    assert "byte limit" in str(error)
else:
    raise AssertionError("oversized API response was accepted")

print("review harvested cases: API responses are bounded before JSON parsing")
