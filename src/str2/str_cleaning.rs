use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use unicode_normalization::UnicodeNormalization;

fn _remove_non_ascii(value: &str, output: &mut String) {
    *output = value.chars().filter(char::is_ascii).collect()
}

#[polars_expr(output_type=String)]
fn remove_non_ascii(inputs: &[Series]) -> PolarsResult<Series> {
    let ca = inputs[0].str()?;
    let out = ca.apply_to_buffer(_remove_non_ascii);

    Ok(out.into_series())
}

fn _remove_diacritics(value: &str, output: &mut String) {
    *output = value.nfd().filter(char::is_ascii).collect()
}

#[polars_expr(output_type=String)]
fn remove_diacritics(inputs: &[Series]) -> PolarsResult<Series> {
    let ca = inputs[0].str()?;
    let out = ca.apply_to_buffer(_remove_diacritics);

    Ok(out.into_series())
}
