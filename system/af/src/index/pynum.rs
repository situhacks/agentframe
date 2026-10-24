//! CPython numerics the ranking depends on: `math.fsum`, `math.sumprod` (3.12, float path),
//! `round(x, n)`, `repr(float)`, and `os.stat().st_mtime`.
//!
//! The scores the index prints and the vector norms it stores are rounded results of these
//! exact algorithms; a naive sum or dot product can move the last bit and, on a near tie,
//! the rank.

/// `math.fsum(xs)`: Shewchuk's exactly rounded sum with CPython's half-even fix-up.
pub fn fsum<I: IntoIterator<Item = f64>>(xs: I) -> f64 {
    let mut p: Vec<f64> = Vec::new();
    for x0 in xs {
        let mut x = x0;
        let mut i = 0;
        for j in 0..p.len() {
            let mut y = p[j];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let hi = x + y;
            let yr = hi - x;
            let lo = y - yr;
            if lo != 0.0 {
                p[i] = lo;
                i += 1;
            }
            x = hi;
        }
        p.truncate(i);
        if x != 0.0 {
            p.push(x);
        }
    }
    let mut n = p.len();
    let mut hi = 0.0;
    if n > 0 {
        n -= 1;
        hi = p[n];
        let mut lo = 0.0;
        while n > 0 {
            let x = hi;
            n -= 1;
            let y = p[n];
            hi = x + y;
            let yr = hi - x;
            lo = y - yr;
            if lo != 0.0 {
                break;
            }
        }
        if n > 0 && ((lo < 0.0 && p[n - 1] < 0.0) || (lo > 0.0 && p[n - 1] > 0.0)) {
            let y = lo * 2.0;
            let x = hi + y;
            let yr = x - hi;
            if y == yr {
                hi = x;
            }
        }
    }
    hi
}

#[derive(Clone, Copy)]
struct Dl {
    hi: f64,
    lo: f64,
}

fn dl_sum(a: f64, b: f64) -> Dl {
    let x = a + b;
    let z = x - a;
    let y = (a - (x - z)) + (b - z);
    Dl { hi: x, lo: y }
}

fn dl_mul(x: f64, y: f64) -> Dl {
    let z = x * y;
    let zz = x.mul_add(y, -z);
    Dl { hi: z, lo: zz }
}

#[derive(Clone, Copy)]
struct Tl {
    hi: f64,
    lo: f64,
    tiny: f64,
}

fn tl_fma(x: f64, y: f64, t: Tl) -> Tl {
    let pr = dl_mul(x, y);
    let sm = dl_sum(t.hi, pr.hi);
    let r1 = dl_sum(t.lo, pr.lo);
    let r2 = dl_sum(r1.hi, sm.lo);
    Tl { hi: sm.hi, lo: r2.hi, tiny: t.tiny + r1.lo + r2.lo }
}

fn tl_to_d(t: Tl) -> f64 {
    let last = dl_sum(t.lo, t.hi);
    t.tiny + last.lo + last.hi
}

/// `math.sumprod(p, q)` over two float sequences (CPython 3.12's triple-length float path);
/// None when the lengths differ (`ValueError: Inputs are not the same length`).
pub fn sumprod(p: &[f32], q: &[f32]) -> Option<f64> {
    if p.len() != q.len() {
        return None;
    }
    let mut total = Tl { hi: 0.0, lo: 0.0, tiny: 0.0 };
    // Python adds each flushed float total to an int 0 (`0 + x`), which only turns -0.0 into 0.0.
    let mut acc = 0.0f64;
    let mut in_use = false;
    for (a, b) in p.iter().zip(q.iter()) {
        let next = tl_fma(*a as f64, *b as f64, total);
        if next.hi.is_finite() {
            total = next;
            in_use = true;
        } else {
            if in_use {
                acc += tl_to_d(total);
                total = Tl { hi: 0.0, lo: 0.0, tiny: 0.0 };
                in_use = false;
            }
            acc += *a as f64 * *b as f64;
        }
    }
    if in_use {
        acc += tl_to_d(total);
    }
    Some(acc)
}

/// `round(x, ndigits)`: correctly rounded to the decimal, half-even, then back to a float.
pub fn py_round(x: f64, ndigits: usize) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let s = format!("{:.*}", ndigits, x);
    let r: f64 = s.parse().unwrap_or(x);
    if r == 0.0 {
        // Python keeps the sign of a value that rounds to zero.
        return if x.is_sign_negative() { -0.0 } else { 0.0 };
    }
    r
}

/// `repr(x)` for a float (also what `json.dumps` writes).
pub fn py_float_repr(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let sci = format!("{:e}", x.abs()); // shortest round-trip digits, d.ddde[-]N
    let (mant, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let decpt = exp + 1;
    let sign = if x < 0.0 { "-" } else { "" };
    let nd = digits.len() as i32;
    if decpt <= -4 || decpt > 16 {
        let m = if nd > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits.clone() };
        let es = if exp < 0 { '-' } else { '+' };
        return format!("{sign}{m}e{es}{:02}", exp.abs());
    }
    let body = if decpt <= 0 {
        format!("0.{}{}", "0".repeat((-decpt) as usize), digits)
    } else if decpt >= nd {
        format!("{}{}.0", digits, "0".repeat((decpt - nd) as usize))
    } else {
        format!("{}.{}", &digits[..decpt as usize], &digits[decpt as usize..])
    };
    format!("{sign}{body}")
}

/// `os.stat(p).st_mtime`: CPython builds it as `sec + nsec * 1e-9`.
pub fn st_mtime(md: &std::fs::Metadata) -> f64 {
    let t = match md.modified() {
        Ok(t) => t,
        Err(_) => return 0.0,
    };
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as f64 + d.subsec_nanos() as f64 * 1e-9,
        Err(e) => {
            // Before 1970: floor seconds, non-negative nanoseconds, as time_t/nsec.
            let d = e.duration();
            let (mut s, mut ns) = (-(d.as_secs() as i64), d.subsec_nanos() as i64);
            if ns > 0 {
                s -= 1;
                ns = 1_000_000_000 - ns;
            }
            s as f64 + ns as f64 * 1e-9
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsum_matches_cpython_on_classic_cases() {
        assert_eq!(fsum([0.1; 10]), 1.0);
        assert_eq!(fsum([1e-16, 1.0, 1e16]), 10000000000000002.0);
        assert_eq!(fsum([1e100, 1.0, -1e100, 1e-100, 1e50, -1.0, -1e50]), 1e-100);
        assert_eq!(fsum(Vec::<f64>::new()), 0.0);
    }

    #[test]
    fn sumprod_and_norm_match_cpython_bit_for_bit() {
        // Reference values from CPython 3.12: math.sumprod over array('f') and
        // math.sqrt(math.fsum(x*x ...)); a naive loop lands a few ulps away.
        for (n, want, naive, norm) in [
            (1024usize, 47.36612647348078, 47.36612647348075, 18.489251113277483),
            (4096, 41.99327157334731, 41.993271573347265, 36.99120593611057),
        ] {
            let a: Vec<f32> = (0..n).map(|i| ((((i * 7919) % 2003) as f64 - 1001.0) / 1001.0) as f32).collect();
            let b: Vec<f32> = (0..n).map(|i| ((((i * 104729) % 1999) as f64 - 999.0) / 997.0) as f32).collect();
            assert_eq!(sumprod(&a, &b), Some(want));
            let mut s = 0.0f64;
            for (x, y) in a.iter().zip(&b) {
                s += *x as f64 * *y as f64;
            }
            assert_eq!(s, naive);
            assert_eq!(fsum(a.iter().map(|x| (*x as f64) * (*x as f64))).sqrt(), norm);
        }
        assert_eq!(sumprod(&[1.0], &[1.0, 2.0]), None);
    }

    #[test]
    fn repr_follows_python() {
        assert_eq!(py_float_repr(1e-5), "1e-05");
        assert_eq!(py_float_repr(0.0001), "0.0001");
        assert_eq!(py_float_repr(1.0), "1.0");
        assert_eq!(py_float_repr(0.032787), "0.032787");
        assert_eq!(py_float_repr(1.5e16), "1.5e+16");
        assert_eq!(py_float_repr(1234567890123456.0), "1234567890123456.0");
        assert_eq!(py_float_repr(-2.5), "-2.5");
    }

    #[test]
    fn round_is_half_even_on_the_exact_value() {
        assert_eq!(py_round(0.0078125, 6), 0.007812);
        assert_eq!(py_round(0.0327868852459, 6), 0.032787);
    }
}
