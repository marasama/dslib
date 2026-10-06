use crate::dslib::csv_parser::Column;
use num_traits::Float;
use std::fmt::Display;

// Count    -> NaN olmayan eleman sayısı
// Mean     -> Welford Online Algorithm (NaN atlanır)
// Std      -> √ Bessel-düzeltmeli Varyans (ddof=1, pandas uyumlu)
// Min/Max  -> NaN atlanır
// %25/%50/%75 -> Pandas uyumlu doğrusal enterpolasyon: q*(N-1)

/// Welford online algoritması: NaN değerleri atlayarak count, mean ve
/// Bessel-düzeltmeli std (ddof=1, pandas uyumlu) hesaplar.
#[allow(unused_assignments)]
fn dslib_welford<F: Float + Display>(rows: &[F]) -> (usize, F, F) {
    let mut count = F::zero();
    let mut mean = F::zero();
    let mut m2 = F::zero();

    for val in rows {
        if val.is_nan() {
            continue; // Boş hücreleri (NaN) atla
        }
        count = count + F::one();
        let delta = *val - mean;
        mean = mean + (delta / count);
        m2 = m2 + (delta * (*val - mean));
    }

    let count_usize = count.to_usize().unwrap_or(0);
    if count_usize < 2 {
        return (count_usize, mean, F::zero());
    }

    let variance = m2 / (count - F::one());
    let std = variance.sqrt();
    (count_usize, mean, std)
}

/// NaN değerleri atlayarak minimum değeri döndürür.
fn dslib_min<F: Float + Display>(rows: &[F]) -> F {
    rows.iter()
        .filter(|x| !x.is_nan())
        .copied()
        .reduce(|a, b| if b < a { b } else { a })
        .unwrap_or_else(F::nan)
}

/// NaN değerleri atlayarak maksimum değeri döndürür.
fn dslib_max<F: Float + Display>(rows: &[F]) -> F {
    rows.iter()
        .filter(|x| !x.is_nan())
        .copied()
        .reduce(|a, b| if b > a { b } else { a })
        .unwrap_or_else(F::nan)
}

/// Pandas uyumlu çeyreklik (quantile) hesabı.
/// - NaN değerler önce süzülür (filtrelenir).
/// - Sanal indeks: idx = q * (N - 1)  [pandas default: linear interpolation]
/// - Güvenli sıralama: NaN kalmadığı için unwrap güvenlidir.
fn dslib_percentage<F: Float + Display>(rows: &[F], percentage: usize) -> F {
    // 1. NaN olmayan geçerli değerleri topla
    let mut clean: Vec<F> = rows.iter().copied().filter(|x| !x.is_nan()).collect();
    let count = clean.len();

    if count == 0 {
        return F::nan();
    }
    if count == 1 {
        return clean[0];
    }

    // 2. Güvenli sıralama (NaN kalmadığı garantilendiğinden unwrap güvenli)
    clean.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // 3. Pandas uyumlu sanal indeks: idx = q * (N - 1)
    let q = F::from(percentage).unwrap() / F::from(100).unwrap();
    let idx = q * F::from(count - 1).unwrap();

    let prev_idx = idx.floor().to_usize().unwrap();
    let after_idx = (prev_idx + 1).min(count - 1); // sınır taşmasını engelle
    let fraction = idx - F::from(prev_idx).unwrap();

    // 4. Doğrusal enterpolasyon: y0 + f * (y1 - y0)
    clean[prev_idx] + fraction * (clean[after_idx] - clean[prev_idx])
}

pub fn process_column<F: Float + Display>(rows: Vec<F>) -> Column<F> {
    let (count, mean, std) = dslib_welford(&rows);
    let min = dslib_min(&rows);
    let max = dslib_max(&rows);
    let per_25 = dslib_percentage(&rows, 25);
    let per_50 = dslib_percentage(&rows, 50);
    let per_75 = dslib_percentage(&rows, 75);
    Column::Number {
        rows,
        count,
        mean,
        std,
        min,
        per_25,
        per_50,
        per_75,
        max,
        range: max - min,
    }
}
