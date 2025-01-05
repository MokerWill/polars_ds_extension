from __future__ import annotations

import polars as pl
import altair as alt
from typing import Iterable, List
from polars._typing import IntoExpr
# Internal dependencies
import polars_ds.sample_and_split as sa
from polars_ds import query_r2, principal_components, query_tpr_fpr, integrate_trapz

alt.data_transformers.enable("vegafusion")

# Plots should never have a title. Title must be editable by the end user
# Interactivity should only be enabled by the end user

def plot_lin_reg(
    df: pl.DataFrame | pl.LazyFrame,
    x: str,
    target: str,
    add_bias: bool = False,
    weights: str | None = None,
    max_points: int = 20_000,
    show_lin_reg_eq: bool = True,
) -> alt.Chart:
    """
    Plots the linear regression line between x and target.

    Paramters
    ---------
    x
        The preditive variable
    target
        The target variable
    add_bias
        Whether to add bias in the linear regression
    weights
        Weights for the linear regression
    max_points
        The max number of points to be displayed. Notice that this only affects the number of points
        on the plot. The linear regression will still be fit on the entire dataset.
    show_lin_reg_eq
        Whether to show the linear regression equation at the bottom or not
    """

    to_select = [x, target] if weights is None else [x, target, weights]
    temp = df.lazy().select(*to_select)

    xx = pl.col(x)
    yy = pl.col(target)
    # Although using simple_lin_reg might seem to be able to reduce some code here,
    # it adds complexity because of output type and the r2 query.
    # A little bit of code dup is reasonable.
    if add_bias:
        if weights is None:
            x_mean = xx.mean()
            y_mean = yy.mean()
            beta = (xx - x_mean).dot(yy - y_mean) / (xx - x_mean).dot(xx - x_mean)
            alpha = y_mean - beta * x_mean
        else:
            w = pl.col(weights)
            w_sum = w.sum()
            x_wmean = w.dot(xx) / w_sum
            y_wmean = w.dot(yy) / w_sum
            beta = w.dot((xx - x_wmean) * (yy - y_wmean)) / (w.dot((xx - x_wmean).pow(2)))
            alpha = y_wmean - beta * x_wmean
    else:
        if weights is None:
            beta = xx.dot(yy) / xx.dot(xx)
        else:
            w = pl.col(weights)
            beta = w.dot(xx * yy) / w.dot(xx.pow(2))

        alpha = pl.lit(0, dtype=pl.Float64)

    beta, alpha, r2, length = (
        temp.select(
            beta.alias("beta"),
            alpha.alias("alpha"),
            query_r2(yy, xx * beta + alpha).alias("r2"),
            pl.len()
        )
        .collect()
        .row(0)
    )

    df_need = temp.select(
        xx,
        yy,
        (xx * beta + alpha).alias("y_pred"),
    )
    # Sample down if len(temp) > max_points
    df_sampled = sa.sample(df_need, value=max_points) if length > max_points else df_need.collect()

    x_title = [x]
    if show_lin_reg_eq:
        if add_bias and alpha > 0:
            reg_info = f"y = {beta:.4f} * x + {round(alpha, 4) if add_bias else ''}, r2 = {r2:.4f}"
        elif add_bias and alpha < 0:
            reg_info = (
                f"y = {beta:.4f} * x - {abs(round(alpha, 4)) if add_bias else ''}, r2 = {r2:.4f}"
            )
        else:
            reg_info = f"y = {beta:.4f} * x, r2 = {r2:.4f}"

        x_title.append(reg_info)

    chart = (
        alt.Chart(df_sampled)
        .mark_point()
        .encode(alt.X(x).scale(zero=False), alt.Y(target))
    )
    return (
        chart 
        + chart.mark_line().encode(
            alt.X(x, title = x_title).scale(zero=False), 
            alt.Y("y_pred"),
        )
    )


def plot_pca(
    df: pl.DataFrame | pl.LazyFrame,
    features: List[str],
    by: IntoExpr,
    center: bool = True,
    dim: int = 2,
    filter_by: pl.Expr | None = None,
    max_points: int = 10_000,
    **kwargs,
) -> alt.Chart:
    """
    Creates a scatter plot based on the reduced dimensions via PCA, and color it by `by`.

    Paramters
    ---------
    df
        Either an eager or lazy Polars Dataframe
    features
        List of feature names
    by
        Color the 2-D PCA plot by the values in the column
    center
        Whether to automatically center the features
    dim
        Only 2 principal components plot can be done at this moment.
    filter_by
        A boolean expression
    max_points
        The max number of points to be displayed. If data > this limit, the data will be sampled.
    kwargs
        Anything else that will be passed to Altair encode function
    """
    if len(features) < 2:
        raise ValueError("You must pass >= 2 features.")
    if dim not in (2, 3):
        raise ValueError("Dim must be 2 or 3.")

    frame = df if filter_by is None else df.filter(filter_by)

    temp = frame.select(principal_components(*features, center=center, k=dim).alias("pc"), by)
    df_plot = sa.sample(temp, value=max_points).unnest("pc")

    if dim == 2:
        return alt.Chart(df_plot).mark_circle(size=60).encode(
            x='pc1',
            y='pc2',
            color=by,
            **kwargs
        ) # .interactive()
    else: # 3d
        raise NotImplementedError

def plot_roc_auc(
    *,
    actual: Iterable[int] | str | pl.Expr,
    pred: Iterable[float] | str | pl.Expr,
    df: pl.DataFrame | pl.LazyFrame | None = None,
    show_auc: bool = True,
    estimator_name: str = "",
    line_color: str = "#92e884",
    round_to: int = 4
) -> alt.Chart:
    """
    Plots ROC AUC curve.

    Paramters
    ---------
    df
        Either an eager or lazy Polars Dataframe
    actual
        A column which has the actual binary target information
    pred
        The prediction
    show_auc
        Whether to show the AUC value or not
    estimator_name
        Name for the estiamtor. Only shown if show_auc is True
    line_color
        HTML color code
    round_to
        Round to n-th decimal digit if show_auc is True
    """
    # expr_based = isinstance(actual, (str, pl.Expr)) and isinstance(pred, (str, pl.Expr)) and isinstance(df, (pl.DataFrame, pl.LazyFrame))
    if isinstance(actual, (str, pl.Expr)) and isinstance(pred, (str, pl.Expr)) and isinstance(df, (pl.DataFrame, pl.LazyFrame)):
        zero = pl.DataFrame({
            "tpr": [0.],
            "fpr": [0.],
        }, schema = {
            "tpr": pl.Float64,
            "fpr": pl.Float64,
        })
        
        tpr_fpr = df.lazy().select(
            tpr_fpr = query_tpr_fpr(actual, pred).reverse()
        ).unnest("tpr_fpr").select(
            "tpr",
            "fpr",
        ).collect()
        df_plot = pl.concat([zero, tpr_fpr])

        chart = alt.Chart(df_plot).mark_line(interpolate="step", color = line_color).encode(
            x=alt.X('fpr', title = "False Positive Rate"),
            y=alt.Y('tpr', title = "True Positive Rate"),
        )
        if show_auc:
            auc = tpr_fpr.select(
                integrate_trapz("tpr", "fpr")
            ).item(0, 0)
            df_text = pl.DataFrame({
                "x": [1.0]
                , "y": [0.]
            })
            estimator = estimator_name.strip()
            auc_text = f"AUC = {round(auc, round_to)}" if estimator == "" else f"{estimator} (AUC = {round(auc, round_to)})"
            text = alt.Chart(df_text).mark_point(opacity=0.0).encode(
                x = alt.X("x"),
                y = alt.Y("y"),
            ).mark_text(
                dx = -1,
                dy = -5,
                fontWeight="bold",
                text = auc_text,
                align="right"
            )
            return chart + text
        else:
            return chart
    else: # May fail. User should catch
        s1 = pl.Series("actual", values=actual, dtype=pl.UInt32)
        s2 = pl.Series("pred", values=pred)
        df_temp = pl.DataFrame({
            "actual": s1,
            "pred": s2, 
        })
        return plot_roc_auc(df = df_temp, actual = "actual", pred = "pred", show_auc=show_auc, estimator_name = estimator_name, line_color=line_color, round_to=round_to)

