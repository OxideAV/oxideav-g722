//! ITU-T G.191 STL basic operators — the 16/32-bit saturating
//! fixed-point operator set the Appendix IV packet-loss concealment is
//! specified against.
//!
//! Semantics transcribed from the staged clean-room notes
//! `docs/audio/g722/basic-operators/stl-basic-operator-semantics.md`,
//! themselves written from the two staged ITU-T G.191 STL manuals
//! (STL2005 chapter 13 / STL2009 chapter 18, "BASOP: ITU-T Basic
//! Operators"). Clause IV.7 of the staged 2012 consolidated
//! Recommendation makes the fixed-point description normative over the
//! prose of clauses IV.5 / IV.6, so *where the rounding constant is
//! added* and *where saturation clamps* is part of the specification.
//!
//! Conventions used here:
//!
//! - 16-bit values travel as `i32` restricted to −32768 ..= 32767
//!   (the crate-wide convention; every operator producing a 16-bit
//!   result saturates into that range).
//! - 32-bit values travel as `i32` over their full range.
//! - Every operator clamps **per operation** at its own output width
//!   (manual §13.2.1: there is no wider accumulator surviving across
//!   operators).
//! - All right shifts are arithmetic (sign-propagating), never
//!   division: `shr(-1, 1) == -1`.

/// Upper 16-bit rail.
pub(crate) const MAX_16: i32 = 32_767;
/// Lower 16-bit rail.
pub(crate) const MIN_16: i32 = -32_768;

#[inline]
fn sat16(x: i32) -> i32 {
    x.clamp(MIN_16, MAX_16)
}

#[inline]
fn sat32(x: i64) -> i32 {
    x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// `add(v1,v2)` — 16-bit addition with saturation (§2.1).
///
/// (Part of the transcribed operator set; the base-codec paths keep
/// their own clause 5.2 operators in `predictor.rs`, so some family
/// members are exercised only by the operator tests today.)
#[allow(dead_code)]
#[inline]
pub(crate) fn add(a: i32, b: i32) -> i32 {
    sat16(a + b)
}

/// `sub(v1,v2)` — 16-bit subtraction with saturation (§2.1).
#[allow(dead_code)]
#[inline]
pub(crate) fn sub(a: i32, b: i32) -> i32 {
    sat16(a - b)
}

/// `abs_s(v1)` — absolute value; `abs_s(-32768) = 32767` (§2.1).
#[inline]
pub(crate) fn abs_s(a: i32) -> i32 {
    sat16(a.abs())
}

/// `negate(v1) = sub(0, v1)` — saturates for −32768 (§2.1).
#[allow(dead_code)]
#[inline]
pub(crate) fn negate(a: i32) -> i32 {
    sub(0, a)
}

/// `shl(v1,v2)` — arithmetic left shift with saturation; negative
/// count shifts right with sign extension (§2.1).
#[inline]
pub(crate) fn shl(a: i32, n: i32) -> i32 {
    if n < 0 {
        shr(a, -n)
    } else if n >= 15 {
        // Any non-zero value shifted ≥ 15 leaves the 16-bit range
        // (except 0, and −1 << 15 == −32768 exactly).
        sat16(i64::from(a).checked_shl(n.min(31) as u32).map_or(
            if a > 0 {
                i64::from(MAX_16)
            } else if a < 0 {
                i64::from(MIN_16)
            } else {
                0
            },
            |v| v.clamp(i64::from(MIN_16), i64::from(MAX_16)),
        ) as i32)
    } else {
        sat16(((i64::from(a)) << n).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }
}

/// `shr(v1,v2)` — arithmetic right shift with sign extension;
/// negative count delegates to [`shl`] (§2.1).
#[inline]
pub(crate) fn shr(a: i32, n: i32) -> i32 {
    if n < 0 {
        shl(a, -n)
    } else if n >= 15 {
        if a < 0 {
            -1
        } else {
            0
        }
    } else {
        a >> n
    }
}

/// `norm_s(v1)` — left shifts needed to normalise a 16-bit value into
/// [16384, 32767] / [−32768, −16384] (§2.1). `norm_s(0) = 0` (the only
/// self-consistent value: `shl(0, n) = 0` for every n).
#[allow(dead_code)]
#[inline]
pub(crate) fn norm_s(a: i32) -> i32 {
    if a == 0 {
        return 0;
    }
    let mut v = a;
    let mut n = 0;
    while (-16_384..16_384).contains(&v) {
        v *= 2;
        n += 1;
    }
    n
}

/// `mult(v1,v2)` — truncating Q15 multiply: `(v1·v2) >> 15` with the
/// arithmetic shift (truncation toward −∞); `mult(-32768,-32768) =
/// 32767` (§2.2).
#[inline]
pub(crate) fn mult(a: i32, b: i32) -> i32 {
    sat16((a * b) >> 15)
}

/// `mult_r(v1,v2)` — Q15 multiply with round-half-up:
/// `((v1·v2) + 16384) >> 15`; `mult_r(-32768,-32768) = 32767` (§2.2).
#[inline]
pub(crate) fn mult_r(a: i32, b: i32) -> i32 {
    sat16((a * b + 16_384) >> 15)
}

/// `L_mult(v1,v2)` — 32-bit result of `v1·v2` with one left shift;
/// the only saturating case is `L_mult(-32768,-32768) = 2147483647`
/// (§2.2).
#[inline]
pub(crate) fn l_mult(a: i32, b: i32) -> i32 {
    sat32(2 * i64::from(a) * i64::from(b))
}

/// `L_mult0(v1,v2)` — same without the left shift; exact, never
/// saturates for 16-bit operands (§2.2).
#[inline]
pub(crate) fn l_mult0(a: i32, b: i32) -> i32 {
    a * b
}

/// `L_add(L_v1,L_v2)` — 32-bit addition with saturation (§2.4).
#[inline]
pub(crate) fn l_add(a: i32, b: i32) -> i32 {
    a.saturating_add(b)
}

/// `L_sub(L_v1,L_v2)` — 32-bit subtraction with saturation (§2.4).
#[inline]
pub(crate) fn l_sub(a: i32, b: i32) -> i32 {
    a.saturating_sub(b)
}

/// `L_abs(L_v1)` — `L_abs(-2147483648) = 2147483647` (§2.4).
#[inline]
pub(crate) fn l_abs(a: i32) -> i32 {
    if a == i32::MIN {
        i32::MAX
    } else {
        a.abs()
    }
}

/// `L_mac(L_v3,v1,v2) = L_add(L_v3, L_mult(v1,v2))` — two saturation
/// points (§2.3).
#[inline]
pub(crate) fn l_mac(acc: i32, a: i32, b: i32) -> i32 {
    l_add(acc, l_mult(a, b))
}

/// `L_mac0(L_v3,v1,v2) = L_add(L_v3, L_mult0(v1,v2))` (§2.3).
#[inline]
pub(crate) fn l_mac0(acc: i32, a: i32, b: i32) -> i32 {
    l_add(acc, l_mult0(a, b))
}

/// `L_msu(L_v3,v1,v2) = L_sub(L_v3, L_mult(v1,v2))` (§2.3).
#[inline]
pub(crate) fn l_msu(acc: i32, a: i32, b: i32) -> i32 {
    l_sub(acc, l_mult(a, b))
}

/// `L_shl(L_v1,v2)` — 32-bit arithmetic left shift with saturation;
/// negative count shifts right (§2.4).
#[inline]
pub(crate) fn l_shl(a: i32, n: i32) -> i32 {
    if n < 0 {
        l_shr(a, -n)
    } else if n >= 31 {
        if a > 0 {
            i32::MAX
        } else if a < 0 {
            i32::MIN
        } else {
            0
        }
    } else {
        sat32(i64::from(a) << n)
    }
}

/// `L_shr(L_v1,v2)` — 32-bit arithmetic right shift with sign
/// extension (§2.4).
#[inline]
pub(crate) fn l_shr(a: i32, n: i32) -> i32 {
    if n < 0 {
        l_shl(a, -n)
    } else if n >= 31 {
        if a < 0 {
            -1
        } else {
            0
        }
    } else {
        a >> n
    }
}

/// `norm_l(L_v1)` — left shifts needed to normalise a 32-bit value
/// into [2^30, 2^31−1] / [−2^31, −2^30] (§2.4). `norm_l(0) = 0`.
#[inline]
pub(crate) fn norm_l(a: i32) -> i32 {
    if a == 0 {
        return 0;
    }
    let mut v = i64::from(a);
    let mut n = 0;
    while (-1_073_741_824..1_073_741_824).contains(&v) {
        v *= 2;
        n += 1;
    }
    n
}

/// `extract_h(L_v1)` — the 16 MSB; a pure bit slice, no saturation
/// (§2.5).
#[inline]
pub(crate) fn extract_h(a: i32) -> i32 {
    a >> 16
}

/// `extract_l(L_v1)` — the 16 LSB, sign-extended; wraps, no
/// saturation (§2.5).
#[inline]
pub(crate) fn extract_l(a: i32) -> i32 {
    i32::from(a as i16)
}

/// `round_fx(L_v1) = extract_h(L_add(L_v1, 32768))` — round half up
/// with the add saturating first (§2.5).
#[inline]
pub(crate) fn round_fx(a: i32) -> i32 {
    extract_h(l_add(a, 32_768))
}

/// `L_deposit_h(v1)` — v1 into the 16 MSB, LSBs zeroed (§2.5).
#[inline]
pub(crate) fn l_deposit_h(a: i32) -> i32 {
    a << 16
}

/// `div_s(v1,v2)` — fractional Q15 division; requires `0 < v1 ≤ v2`
/// (the caller pre-normalises); the result is positive and truncated;
/// `div_s(v1,v1) = 32767` (§2.7).
#[inline]
pub(crate) fn div_s(a: i32, b: i32) -> i32 {
    debug_assert!(
        a >= 0 && b > 0 && a <= b,
        "div_s precondition: 0 < v1 <= v2"
    );
    if a == b {
        MAX_16
    } else {
        (((i64::from(a)) << 15) / i64::from(b)) as i32
    }
}

/// The double-precision (hi, lo) split of a 32-bit value, per the
/// encoding pinned by staged data (`docs/audio/g722/basic-operators/
/// stl-basic-operator-semantics.md` §4): `L = (hi << 16) + (lo << 1)`,
/// i.e. `hi = L >> 16` and `lo = (L − (hi << 16)) >> 1` — both halves
/// non-negative-lo 15-bit words. Verified against the staged lag-window
/// pairs (`docs/audio/g722/tables/appendix-IV-lag-window-{high,low}`).
#[inline]
pub(crate) fn l_extract(a: i32) -> (i32, i32) {
    let hi = extract_h(a);
    let lo = extract_l(l_shr(l_sub(a, l_deposit_h(hi)), 1));
    (hi, lo)
}

/// Recombine an [`l_extract`] pair: `L = (hi << 16) + (lo << 1)`.
#[inline]
pub(crate) fn l_comp(hi: i32, lo: i32) -> i32 {
    l_add(l_deposit_h(hi), l_shl(lo, 1))
}

/// Double-precision multiply of two `(hi, lo)` pairs, derived from the
/// pinned encoding: with `L/2^31 = hi/2^15 + lo/2^30`, the Q31 product
/// is `2·hi1·hi2 + (hi1·lo2)/2^14 + (lo1·hi2)/2^14` up to the dropped
/// `lo1·lo2` term (below one LSB), realised on the operator set as
/// `L_mult` + two truncating `mult` + `L_mac` steps.
///
/// The staged manuals do not specify the double-precision helper layer
/// (semantics doc §4 records it as a known boundary); this
/// construction is the arithmetic identity implied by the staged
/// encoding, using only staged operators.
#[inline]
pub(crate) fn mpy_32(hi1: i32, lo1: i32, hi2: i32, lo2: i32) -> i32 {
    let mut l = l_mult(hi1, hi2);
    l = l_mac(l, mult(hi1, lo2), 1);
    l = l_mac(l, mult(lo1, hi2), 1);
    l
}

/// Double-precision × single-precision multiply: `(hi, lo) × v` in
/// Q15, same derivation as [`mpy_32`] with `lo2 = 0`.
#[inline]
pub(crate) fn mpy_32_16(hi: i32, lo: i32, v: i32) -> i32 {
    let mut l = l_mult(hi, v);
    l = l_mac(l, mult(lo, v), 1);
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_pinned_corner_cases() {
        // §2.1
        assert_eq!(add(30_000, 30_000), 32_767);
        assert_eq!(add(-30_000, -30_000), -32_768);
        assert_eq!(sub(-30_000, 30_000), -32_768);
        assert_eq!(abs_s(-32_768), 32_767);
        assert_eq!(negate(-32_768), 32_767);
        // Right shift is arithmetic, not division.
        assert_eq!(shr(-1, 1), -1);
        assert_eq!(shr(-3, 1), -2);
        assert_eq!(shl(-32_768, 1), -32_768);
        assert_eq!(shl(16_384, 1), 32_767);
        assert_eq!(shl(1, -2), 0);
        // §2.2
        assert_eq!(l_mult(-32_768, -32_768), 2_147_483_647);
        assert_eq!(l_mult(-32_768, 32_767), 2 * -32_768 * 32_767);
        assert_eq!(mult(-32_768, -32_768), 32_767);
        assert_eq!(mult_r(-32_768, -32_768), 32_767);
        // mult truncates toward −∞; mult_r rounds half up.
        assert_eq!(mult(-3, 16_384), -2);
        assert_eq!(mult_r(-3, 16_384), -1);
        assert_eq!(mult_r(1, 32_767), 1);
        // §2.3 — the accumulator saturates on every L_mac.
        assert_eq!(l_mac(i32::MAX, 1, 1), i32::MAX);
        assert_eq!(l_mac0(i32::MAX - 1, 1, 1), i32::MAX);
        assert_eq!(l_msu(i32::MIN, 1, 1), i32::MIN);
        // §2.4
        assert_eq!(l_add(i32::MAX, 1), i32::MAX);
        assert_eq!(l_sub(i32::MIN, 1), i32::MIN);
        assert_eq!(l_abs(i32::MIN), i32::MAX);
        assert_eq!(l_shl(1 << 30, 1), i32::MAX);
        assert_eq!(l_shr(-1, 5), -1);
        // §2.5 — round_fx(0x00008000) = 1, round_fx(0xFFFF8000) = 0.
        assert_eq!(round_fx(0x0000_8000), 1);
        assert_eq!(round_fx(-32_768), 0);
        assert_eq!(round_fx(0x0000_7FFF), 0);
        assert_eq!(round_fx(i32::MAX), 32_767);
        assert_eq!(extract_l(0x0001_8000), -32_768);
        assert_eq!(extract_h(0xFFFF_0001_u32 as i32), -1);
        // §2.7
        assert_eq!(div_s(5, 5), 32_767);
        assert_eq!(div_s(1, 2), 16_384);
        assert_eq!(div_s(1, 3), 10_922);
    }

    #[test]
    fn normalisation_ranges() {
        assert_eq!(norm_s(0), 0);
        assert_eq!(norm_s(1), 14);
        assert_eq!(norm_s(16_384), 0);
        assert_eq!(norm_s(-1), 15);
        assert_eq!(norm_s(-32_768), 0);
        assert!(shl(23, norm_s(23)) >= 16_384);
        assert_eq!(norm_l(0), 0);
        assert_eq!(norm_l(1), 30);
        assert_eq!(norm_l(-1), 31);
        assert_eq!(norm_l(i32::MAX), 0);
        assert_eq!(norm_l(i32::MIN), 0);
        for v in [3, 1000, 123_456_789, -7, -65_536] {
            let n = norm_l(v);
            let x = l_shl(v, n);
            assert!(
                !(-1_073_741_824..1_073_741_824).contains(&x),
                "norm_l({v}) = {n} does not normalise"
            );
        }
    }

    #[test]
    fn double_precision_encoding_round_trips() {
        // The (hi << 16) + (lo << 1) encoding pinned by the staged
        // lag-window data must round-trip through l_extract/l_comp
        // for every non-negative value (lo is a 15-bit word), losing
        // only the LSB.
        for v in [0, 1, 2, 65_535, 65_536, 0x1234_5678, i32::MAX - 1] {
            let (hi, lo) = l_extract(v);
            assert!((0..32_768).contains(&lo), "lo out of 15-bit range for {v}");
            assert_eq!(l_comp(hi, lo), v & !1, "round trip for {v}");
        }
        // Negative values keep the arithmetic identity as well.
        for v in [-2, -65_536, -0x1234_5678] {
            let (hi, lo) = l_extract(v);
            assert_eq!(l_comp(hi, lo), v & !1, "round trip for {v}");
        }
    }

    #[test]
    fn mpy_32_matches_the_real_product_to_two_lsb() {
        let cases: [(i64, i64); 5] = [
            (0x7FFF_0000, 0x7FFF_0000),
            (0x4000_0000, 0x4000_0000),
            (0x1234_5678, 0x0FED_CBA8),
            (0x0002_0000, 0x7FFF_FFFE),
            (0x4000_0000, 0x0000_1000),
        ];
        for (x, y) in cases {
            let (h1, l1) = l_extract(x as i32);
            let (h2, l2) = l_extract(y as i32);
            let got = i64::from(mpy_32(h1, l1, h2, l2));
            let want = (x * y) >> 31;
            assert!(
                (want - got).abs() <= 2,
                "mpy_32({x:#x}, {y:#x}) = {got:#x}, want ≈ {want:#x}"
            );
        }
    }
}
