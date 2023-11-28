use polars::prelude::{arity::binary_elementwise_values, *};
use pyo3_polars::{
    derive::polars_expr,
    export::polars_core::utils::rayon::prelude::{IndexedParallelIterator, ParallelIterator},
};
use rapidfuzz::distance::{damerau_levenshtein, levenshtein};

#[inline]
fn levenshtein(s1: &str, s2: &str, bound: Option<usize>) -> Option<u32> {
    levenshtein::distance(s1.chars(), s2.chars(), None, bound, None)
        .map_or(None, |u| Some(u as u32))
}

#[inline]
fn levenshtein_sim(s1: &str, s2: &str) -> Option<f64> {
    levenshtein::normalized_similarity(s1.chars(), s2.chars(), None, None, None)
}

#[inline]
fn d_levenshtein(s1: &str, s2: &str) -> Option<u32> {
    damerau_levenshtein::distance(s1.chars(), s2.chars(), None, None)
        .map_or(None, |u| Some(u as u32))
}

#[inline]
fn d_levenshtein_sim(s1: &str, s2: &str) -> Option<f64> {
    damerau_levenshtein::normalized_similarity(s1.chars(), s2.chars(), None, None)
}

#[polars_expr(output_type=UInt32)]
fn pl_levenshtein(inputs: &[Series]) -> PolarsResult<Series> {
    let ca1 = inputs[0].utf8()?;
    let ca2 = inputs[1].utf8()?;
    let parallel = inputs[2].bool()?;
    let parallel = parallel.get(0).unwrap();
    if ca2.len() == 1 {
        let r = ca2.get(0).unwrap();
        let batched = levenshtein::BatchComparator::new(r.chars(), None);
        let out: UInt32Chunked = if parallel {
            ca1.par_iter()
                .map(|op_s| {
                    let s = op_s?;
                    batched
                        .distance(s.chars(), None, None)
                        .map_or(None, |u| Some(u as u32))
                })
                .collect()
        } else {
            ca1.apply_generic(|op_s| {
                let s = op_s?;
                batched
                    .distance(s.chars(), None, None)
                    .map_or(None, |u| Some(u as u32))
            })
        };
        Ok(out.into_series())
    } else if ca1.len() == ca2.len() {
        let out: UInt32Chunked = if parallel {
            ca1.par_iter_indexed()
                .zip(ca2.par_iter_indexed())
                .map(|(op_w1, op_w2)| {
                    let (w1, w2) = (op_w1?, op_w2?);
                    levenshtein(w1, w2, None)
                })
                .collect()
        } else {
            binary_elementwise_values(ca1, ca2, |x, y| levenshtein(x, y, None))
        };
        Ok(out.into_series())
    } else {
        Err(PolarsError::ShapeMismatch(
            "Inputs must have the same length.".into(),
        ))
    }
}

#[polars_expr(output_type=Boolean)]
fn pl_levenshtein_within(inputs: &[Series]) -> PolarsResult<Series> {
    let ca1 = inputs[0].utf8()?;
    let ca2 = inputs[1].utf8()?;
    let bound = inputs[2].u32()?;
    let bound = bound.get(0).map_or_else(|| None, |x| Some(x as usize));
    let parallel = inputs[3].bool()?;
    let parallel = parallel.get(0).unwrap();
    if ca2.len() == 1 {
        let r = ca2.get(0).unwrap();
        let batched = levenshtein::BatchComparator::new(r.chars(), None);
        let out: BooleanChunked = if parallel {
            ca1.par_iter()
                .map(|op_s| {
                    let s = op_s?;
                    Some(batched.distance(s.chars(), bound, None).is_some())
                })
                .collect()
        } else {
            ca1.apply_nonnull_values_generic(DataType::Boolean, |s| {
                batched.distance(s.chars(), bound, None).is_some()
            })
        };
        Ok(out.into_series())
    } else if ca1.len() == ca2.len() {
        let out: BooleanChunked = if parallel {
            ca1.par_iter_indexed()
                .zip(ca2.par_iter_indexed())
                .map(|(op_w1, op_w2)| {
                    let (w1, w2) = (op_w1?, op_w2?);
                    Some(levenshtein(w1, w2, bound).is_some())
                })
                .collect()
        } else {
            binary_elementwise_values(ca1, ca2, |x, y| levenshtein(x, y, bound).is_some())
        };
        Ok(out.into_series())
    } else {
        Err(PolarsError::ShapeMismatch(
            "Inputs must have the same length.".into(),
        ))
    }
}

#[polars_expr(output_type=Float64)]
fn pl_levenshtein_sim(inputs: &[Series]) -> PolarsResult<Series> {
    let ca1 = inputs[0].utf8()?;
    let ca2 = inputs[1].utf8()?;
    let parallel = inputs[2].bool()?;
    let parallel = parallel.get(0).unwrap();
    if ca2.len() == 1 {
        let r = ca2.get(0).unwrap();
        let batched = levenshtein::BatchComparator::new(r.chars(), None);
        let out: Float64Chunked = if parallel {
            ca1.par_iter()
                .map(|op_s| {
                    let s = op_s?;
                    batched.normalized_similarity(s.chars(), None, None)
                })
                .collect()
        } else {
            ca1.apply_generic(|op_s| {
                let s = op_s?;
                batched.normalized_similarity(s.chars(), None, None)
            })
        };
        Ok(out.into_series())
    } else if ca1.len() == ca2.len() {
        let out: Float64Chunked = if parallel {
            ca1.par_iter_indexed()
                .zip(ca2.par_iter_indexed())
                .map(|(op_w1, op_w2)| {
                    let (w1, w2) = (op_w1?, op_w2?);
                    levenshtein_sim(w1, w2)
                })
                .collect()
        } else {
            binary_elementwise_values(ca1, ca2, levenshtein_sim)
        };
        Ok(out.into_series())
    } else {
        Err(PolarsError::ShapeMismatch(
            "Inputs must have the same length.".into(),
        ))
    }
}

#[polars_expr(output_type=UInt32)]
fn pl_d_levenshtein(inputs: &[Series]) -> PolarsResult<Series> {
    let ca1 = inputs[0].utf8()?;
    let ca2 = inputs[1].utf8()?;
    let parallel = inputs[2].bool()?;
    let parallel = parallel.get(0).unwrap();
    if ca2.len() == 1 {
        let r = ca2.get(0).unwrap();
        let batched = damerau_levenshtein::BatchComparator::new(r.chars());
        let out: UInt32Chunked = if parallel {
            ca1.par_iter()
                .map(|op_s| {
                    let s = op_s?;
                    batched
                        .distance(s.chars(), None, None)
                        .map_or(None, |u| Some(u as u32))
                })
                .collect()
        } else {
            ca1.apply_generic(|op_s| {
                let s = op_s?;
                batched
                    .distance(s.chars(), None, None)
                    .map_or(None, |u| Some(u as u32))
            })
        };
        Ok(out.into_series())
    } else if ca1.len() == ca2.len() {
        let out: UInt32Chunked = if parallel {
            ca1.par_iter_indexed()
                .zip(ca2.par_iter_indexed())
                .map(|(op_w1, op_w2)| {
                    let (w1, w2) = (op_w1?, op_w2?);
                    d_levenshtein(w1, w2)
                })
                .collect()
        } else {
            binary_elementwise_values(ca1, ca2, d_levenshtein)
        };
        Ok(out.into_series())
    } else {
        Err(PolarsError::ShapeMismatch(
            "Inputs must have the same length.".into(),
        ))
    }
}

#[polars_expr(output_type=Float64)]
fn pl_d_levenshtein_sim(inputs: &[Series]) -> PolarsResult<Series> {
    let ca1 = inputs[0].utf8()?;
    let ca2 = inputs[1].utf8()?;
    let parallel = inputs[2].bool()?;
    let parallel = parallel.get(0).unwrap();
    if ca2.len() == 1 {
        let r = ca2.get(0).unwrap();
        let batched = damerau_levenshtein::BatchComparator::new(r.chars());
        let out: Float64Chunked = if parallel {
            ca1.par_iter()
                .map(|op_s| {
                    let s = op_s?;
                    batched.normalized_similarity(s.chars(), None, None)
                })
                .collect()
        } else {
            ca1.apply_generic(|op_s| {
                let s = op_s?;
                batched.normalized_similarity(s.chars(), None, None)
            })
        };
        Ok(out.into_series())
    } else if ca1.len() == ca2.len() {
        let out: Float64Chunked = if parallel {
            ca1.par_iter_indexed()
                .zip(ca2.par_iter_indexed())
                .map(|(op_w1, op_w2)| {
                    let (w1, w2) = (op_w1?, op_w2?);
                    d_levenshtein_sim(w1, w2)
                })
                .collect()
        } else {
            binary_elementwise_values(ca1, ca2, d_levenshtein_sim)
        };
        Ok(out.into_series())
    } else {
        Err(PolarsError::ShapeMismatch(
            "Inputs must have the same length.".into(),
        ))
    }
}
