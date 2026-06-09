"""Unit tests for the correctness-gate helper ``assert_agree`` (Task 3).

``assert_agree`` aligns two ``{poly_id: float}`` result dicts on their
overlapping poly_ids, compares them NaN-aware, and returns a report
``{agree, max_abs_diff, n_compared, n_mismatch, examples}``.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))


def test_identical_dicts_agree():
    from scripts.bench_zonal_headtohead import assert_agree

    a = {0: 1.0, 1: 2.0, 2: 3.0}
    rep = assert_agree(a, dict(a), "a", "b", abs_tol=1e-6)
    assert rep["agree"] is True
    assert rep["max_abs_diff"] == 0.0
    assert rep["n_compared"] == 3
    assert rep["n_mismatch"] == 0
    assert rep["examples"] == []


def test_aligns_on_overlap_only():
    from scripts.bench_zonal_headtohead import assert_agree

    a = {0: 1.0, 1: 2.0, 9: 99.0}
    b = {0: 1.0, 1: 2.0, 7: -1.0}
    rep = assert_agree(a, b, "a", "b", abs_tol=1e-6)
    # Only ids 0 and 1 overlap; the disjoint ids are ignored.
    assert rep["n_compared"] == 2
    assert rep["agree"] is True


def test_difference_above_tol_is_mismatch():
    from scripts.bench_zonal_headtohead import assert_agree

    a = {0: 1.0, 1: 2.0}
    b = {0: 1.0, 1: 2.5}
    rep = assert_agree(a, b, "a", "b", abs_tol=0.1)
    assert rep["agree"] is False
    assert rep["n_mismatch"] == 1
    assert math.isclose(rep["max_abs_diff"], 0.5)
    assert rep["examples"][0][0] == 1


def test_both_nan_is_not_a_mismatch():
    from scripts.bench_zonal_headtohead import assert_agree

    nan = float("nan")
    a = {0: nan, 1: 5.0}
    b = {0: nan, 1: 5.0}
    rep = assert_agree(a, b, "a", "b", abs_tol=1e-6)
    assert rep["agree"] is True
    assert rep["n_mismatch"] == 0


def test_one_nan_one_finite_is_a_mismatch():
    from scripts.bench_zonal_headtohead import assert_agree

    nan = float("nan")
    a = {0: nan}
    b = {0: 5.0}
    rep = assert_agree(a, b, "a", "b", abs_tol=1e-6)
    assert rep["agree"] is False
    assert rep["n_mismatch"] == 1


def test_no_overlap_raises():
    from scripts.bench_zonal_headtohead import assert_agree

    import pytest

    with pytest.raises(AssertionError):
        assert_agree({0: 1.0}, {1: 1.0}, "a", "b", abs_tol=1e-6)
