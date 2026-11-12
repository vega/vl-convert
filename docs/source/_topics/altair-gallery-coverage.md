---
title: PGFPlots gallery coverage
path: guides/pgfplots-gallery
section: Advanced
order: 350
interfaces: [python, cli, rust]
---

<!-- topic-body -->

# Altair gallery → native PGFPlots: capability registry

Pinned catalog: **188 unique examples**, from Altair gallery manifest blob `130022d1ad70537f1be96a7e5374818224904b99`. Generated 2026-11-11.

**This is a static capability audit, not an executed validation of the 188 gallery specifications.** In particular, no item is marked `verified` until its actual Altair-emitted spec is compiled through the Rust/Python/CLI target and the TeX PDF is reviewed.

Counts: `excluded_map` **14**; `native_family_candidate` **56**; `compiler_gap` **28**; `preprocessing_or_adapter` **56**; `excluded_interactive` **34**.

## Executable conformance

The table below is the **historical planning baseline**, not current test results.
Run the `gallery-conformance` job in **Native PGFPlots semantics and TeX** for
source-pinned observed results. Its `altair-pgfplots-conformance` artifact contains
`outcomes.json`, `summary.json`, per-case original and normalized specifications,
reference SVG, native TeX/PDF, and an HTML index of paired images. Accepted-case
regressions, unobserved cases, source failures, and reference-renderer failures
fail CI. Unsupported graphical features remain explicit outcomes; they do not
become compatibility passes.

The built PyO3 extension and CLI are checked against the identical production
JavaScript modules, and the rendering pass compiles the actual extension output.
Numeric probes and human visual approval are separate fields; rendering alone
does not set either to passed. See [Native PGFPlots export](pgfplots.md).

## Historical status contract

- `excluded_map`: outside requested scope.
- `excluded_interactive`: interaction intrinsic to example; outside requested scope.
- `native_family_candidate`: same kind of mark and axes exists, but exact example untested.
- `preprocessing_or_adapter`: requires materializing data/transforms/temporal fields or rewriting the spec.
- `compiler_gap`: current compiler lacks required semantics/styling, not just a test.

Every candidate needs an actual source-spec and rendered-PDF test before being called supported. See [machine-readable JSON registry](https://github.com/marcusinthesky/vl-convert/blob/feature/experimental-pgfplots-export/docs/source/_topics/altair_gallery_registry.json).

| Gallery example | Status | Main blocker / qualification |
|---|---|---|
| [Connections Among U.S. Airports Interactive ](https://altair-viz.github.io/gallery/airport_connections.html) | `excluded_map` | geographic/geoshape intent |
| [Andrews Curves ](https://altair-viz.github.io/gallery/andrews_curves.html) | `native_family_candidate` | inline data required |
| [Annual Weather Heatmap ](https://altair-viz.github.io/gallery/annual_weather_heatmap.html) | `native_family_candidate` | inline data required |
| [Anscombe's Quartet ](https://altair-viz.github.io/gallery/anscombe_plot.html) | `native_family_candidate` | inline data required |
| [Area Chart with Gradient ](https://altair-viz.github.io/gallery/area_chart_gradient.html) | `compiler_gap` | gradient fills; unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Faceted Area Chart ](https://altair-viz.github.io/gallery/area_faceted.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Bar Chart with Line on Dual Axis ](https://altair-viz.github.io/gallery/bar_and_line_with_dual_axis.html) | `compiler_gap` | independent dual-axis layers |
| [Compact Faceted Grid of Bar Charts ](https://altair-viz.github.io/gallery/bar_chart_faceted_compact.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Horizontal Bar Chart ](https://altair-viz.github.io/gallery/bar_chart_horizontal.html) | `native_family_candidate` | inline data required |
| [Sorted Bar Chart ](https://altair-viz.github.io/gallery/bar_chart_sorted.html) | `native_family_candidate` | inline data required |
| [Bar Chart with Highlighted Bar ](https://altair-viz.github.io/gallery/bar_chart_with_highlighted_bar.html) | `native_family_candidate` | inline data required |
| [Bar Chart with Highlighted Segment ](https://altair-viz.github.io/gallery/bar_chart_with_highlighted_segment.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate |
| [Bar Chart with Labels ](https://altair-viz.github.io/gallery/bar_chart_with_labels.html) | `native_family_candidate` | inline data required |
| [Bar Chart with Labels based on Measured Luminance ](https://altair-viz.github.io/gallery/bar_chart_with_labels_measured_luminance.html) | `compiler_gap` | expression-driven text color |
| [Bar Chart with Line at Mean ](https://altair-viz.github.io/gallery/bar_chart_with_mean_line.html) | `native_family_candidate` | inline data required |
| [Bar Chart with Negative Values ](https://altair-viz.github.io/gallery/bar_chart_with_negatives.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Bar Chart with Range ](https://altair-viz.github.io/gallery/bar_chart_with_range.html) | `native_family_candidate` | inline data required |
| [Bar Chart Highlighting Values beyond a Threshold ](https://altair-viz.github.io/gallery/bar_chart_with_single_threshold.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Faceted Stacked Bar Chart ](https://altair-viz.github.io/gallery/bar_faceted_stacked.html) | `native_family_candidate` | inline data required |
| [Bar Chart with Rounded Edges ](https://altair-viz.github.io/gallery/bar_rounded.html) | `compiler_gap` | rounded bar corners |
| [Bar Chart with Rolling Mean ](https://altair-viz.github.io/gallery/bar_with_rolling_mean.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Becker's Barley Faceted Plot ](https://altair-viz.github.io/gallery/beckers_barley_facet.html) | `native_family_candidate` | inline data required |
| [Becker's Barley Wrapped Facet Plot ](https://altair-viz.github.io/gallery/beckers_barley_wrapped_facet.html) | `native_family_candidate` | inline data required |
| [Boxplot with Min/Max Whiskers ](https://altair-viz.github.io/gallery/boxplot.html) | `preprocessing_or_adapter` | min-max whisker semantics differ from default |
| [Bubble Plot ](https://altair-viz.github.io/gallery/bubble_plot.html) | `compiler_gap` | variable symbol sizes require review |
| [Bump Chart ](https://altair-viz.github.io/gallery/bump_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Calculate Residuals ](https://altair-viz.github.io/gallery/calculate_residuals.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:joinaggregate, transform:timeunit; temporal fields/time units require materialization |
| [Candlestick Chart ](https://altair-viz.github.io/gallery/candlestick_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Choropleth Map ](https://altair-viz.github.io/gallery/choropleth.html) | `excluded_map` | geographic/geoshape intent |
| [Repeated Choropleth Map ](https://altair-viz.github.io/gallery/choropleth_repeat.html) | `excluded_map` | geographic/geoshape intent |
| [Atmospheric CO2 Concentration ](https://altair-viz.github.io/gallery/co2_concentration.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:window |
| [Comet Chart ](https://altair-viz.github.io/gallery/comet_chart.html) | `compiler_gap` | unsupported marks: mark:trail; unimplemented transform semantics: transform:calculate, transform:pivot |
| [Cumulative Count Chart ](https://altair-viz.github.io/gallery/cumulative_count_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Dendrogram of Hierarchical Clustering ](https://altair-viz.github.io/gallery/dendrogram.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Repeated Density Estimates ](https://altair-viz.github.io/gallery/density_repeat.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:density |
| [Stacked Density Estimates ](https://altair-viz.github.io/gallery/density_stack.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:density |
| [Confidence Interval Ellipses ](https://altair-viz.github.io/gallery/deviation_ellipses.html) | `compiler_gap` | custom ellipse geometry |
| [Distributions and Medians of Likert Scale Ratings ](https://altair-viz.github.io/gallery/distributions_and_medians_of_likert_scale_ratings.html) | `compiler_gap` | unsupported marks: mark:tick |
| [Faceted Histogram ](https://altair-viz.github.io/gallery/distributions_faceted_histogram.html) | `native_family_candidate` | inline data required |
| [Diverging Stacked Bar Chart ](https://altair-viz.github.io/gallery/diverging_stacked_bar_chart.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Donut Chart ](https://altair-viz.github.io/gallery/donut_chart.html) | `compiler_gap` | unsupported marks: mark:arc |
| [Dot Dash Plot ](https://altair-viz.github.io/gallery/dot_dash_plot.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Empirical Cumulative Distribution Function ](https://altair-viz.github.io/gallery/empirical_cumulative_distribution_function.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Error Bars with Confidence Interval ](https://altair-viz.github.io/gallery/errorbars_with_ci.html) | `preprocessing_or_adapter` | bootstrap confidence interval must be precomputed |
| [Error Bars with Standard Deviation ](https://altair-viz.github.io/gallery/errorbars_with_std.html) | `native_family_candidate` | inline data required |
| [Population of Falkensee from 1875 to 2014 ](https://altair-viz.github.io/gallery/falkensee.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Filled Step Chart ](https://altair-viz.github.io/gallery/filled_step_chart.html) | `native_family_candidate` | inline data required |
| [Gantt Chart ](https://altair-viz.github.io/gallery/gantt_chart.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Gapminder Bubble Plot ](https://altair-viz.github.io/gallery/gapminder_bubble_plot.html) | `native_family_candidate` | inline data required |
| [Grouped Points with Proportional Symbols Map ](https://altair-viz.github.io/gallery/groupby-map.html) | `excluded_map` | geographic/geoshape intent |
| [Grouped Bar Chart ](https://altair-viz.github.io/gallery/grouped_bar_chart.html) | `native_family_candidate` | inline data required |
| [Grouped Bar Chart with xOffset ](https://altair-viz.github.io/gallery/grouped_bar_chart2.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Horizontal Grouped Bar Chart ](https://altair-viz.github.io/gallery/grouped_bar_chart_horizontal.html) | `native_family_candidate` | inline data required |
| [Grouped Bar Chart with xOffset and overlapping bars ](https://altair-viz.github.io/gallery/grouped_bar_chart_overlapping_bars.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Grouped Bar Chart with Error Bars ](https://altair-viz.github.io/gallery/grouped_bar_chart_with_error_bars.html) | `native_family_candidate` | inline data required |
| [Heat Lane Chart ](https://altair-viz.github.io/gallery/heat_lane.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:joinaggregate; bins may need explicit step/extent |
| [Hexbin Chart ](https://altair-viz.github.io/gallery/hexbins.html) | `compiler_gap` | custom hexagonal path symbols; unimplemented transform semantics: transform:calculate |
| [Histogram with Gradient Color ](https://altair-viz.github.io/gallery/histogram_gradient_color.html) | `compiler_gap` | gradient fills |
| [2D Histogram Heatmap ](https://altair-viz.github.io/gallery/histogram_heatmap.html) | `native_family_candidate` | inline data required |
| [Histogram with Responsive Bins ](https://altair-viz.github.io/gallery/histogram_responsive.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [2D Histogram Scatter Plot ](https://altair-viz.github.io/gallery/histogram_scatterplot.html) | `native_family_candidate` | inline data required |
| [Histogram with a Global Mean Overlay ](https://altair-viz.github.io/gallery/histogram_with_a_global_mean_overlay.html) | `native_family_candidate` | inline data required |
| [Horizon Graph ](https://altair-viz.github.io/gallery/horizon_graph.html) | `compiler_gap` | overlapping clipped area bands; unimplemented transform semantics: transform:calculate |
| [Horizontal Stacked Bar Chart ](https://altair-viz.github.io/gallery/horizontal_stacked_bar_chart.html) | `native_family_candidate` | inline data required |
| [Interactive Chart with Aggregation ](https://altair-viz.github.io/gallery/interactive_aggregation.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Bar Chart with Highlighting on Hover and Selection on Click ](https://altair-viz.github.io/gallery/interactive_bar_select_highlight.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Histogram with Adjustable Bin Size ](https://altair-viz.github.io/gallery/interactive_bin_size.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interactive Rectangular Brush ](https://altair-viz.github.io/gallery/interactive_brush.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interactive Selection of Columns ](https://altair-viz.github.io/gallery/interactive_column_selection.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interactive Chart with Cross-Highlight ](https://altair-viz.github.io/gallery/interactive_cross_highlight.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Filter a Time Series with Date and Number Inputs ](https://altair-viz.github.io/gallery/interactive_date_input.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interactive Crossfilter ](https://altair-viz.github.io/gallery/interactive_layered_crossfilter.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interactive Legend ](https://altair-viz.github.io/gallery/interactive_legend.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Reorder stacked bar segments ](https://altair-viz.github.io/gallery/interactive_reorder_stacked_bars.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Simple Interactive Colored Scatter Plot ](https://altair-viz.github.io/gallery/interactive_scatter_plot.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interval Selection with Initial Date Range ](https://altair-viz.github.io/gallery/interval_selection.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interval Selection on a Map ](https://altair-viz.github.io/gallery/interval_selection_map_quakes.html) | `excluded_map` | geographic/geoshape intent |
| [Iowa's Renewable Energy Boom ](https://altair-viz.github.io/gallery/iowa_electricity.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Isotype Visualization ](https://altair-viz.github.io/gallery/isotype.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Isotype Visualization with Emoji ](https://altair-viz.github.io/gallery/isotype_emoji.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:window |
| [Isotype Grid ](https://altair-viz.github.io/gallery/isotype_grid.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate |
| [Lasagna Plot (Dense Time-Series Heatmap) ](https://altair-viz.github.io/gallery/lasagna_plot.html) | `native_family_candidate` | inline data required |
| [Line Chart with Layered Aggregates ](https://altair-viz.github.io/gallery/layer_line_color_rule.html) | `native_family_candidate` | inline data required |
| [Layered Area Chart ](https://altair-viz.github.io/gallery/layered_area_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Layered Bar Chart ](https://altair-viz.github.io/gallery/layered_bar_chart.html) | `native_family_candidate` | inline data required |
| [Bar and Tick Chart ](https://altair-viz.github.io/gallery/layered_chart_bar_mark.html) | `compiler_gap` | unsupported marks: mark:tick |
| [Layered chart with Dual-Axis ](https://altair-viz.github.io/gallery/layered_chart_with_dual_axis.html) | `compiler_gap` | independent dual-axis layers |
| [Text over a Heatmap ](https://altair-viz.github.io/gallery/layered_heatmap_text.html) | `native_family_candidate` | inline data required |
| [Layered Histogram ](https://altair-viz.github.io/gallery/layered_histogram.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Line Chart with Arrows ](https://altair-viz.github.io/gallery/line_chart_with_arrows.html) | `compiler_gap` | arrowhead point shapes |
| [Line Chart with Datum for Color ](https://altair-viz.github.io/gallery/line_chart_with_color_datum.html) | `native_family_candidate` | inline data required |
| [Line Chart with Cumulative Sum ](https://altair-viz.github.io/gallery/line_chart_with_cumsum.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Faceted Line Chart with Cumulative Sum ](https://altair-viz.github.io/gallery/line_chart_with_cumsum_faceted.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Line Chart with Custom Legend ](https://altair-viz.github.io/gallery/line_chart_with_custom_legend.html) | `preprocessing_or_adapter` | custom legend layout; unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Line Chart with Datum ](https://altair-viz.github.io/gallery/line_chart_with_datum.html) | `native_family_candidate` | inline data required |
| [Line Chart with Sequence Generator ](https://altair-viz.github.io/gallery/line_chart_with_generator.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate |
| [Line Chart with Interpolation ](https://altair-viz.github.io/gallery/line_chart_with_interpolation.html) | `compiler_gap` | monotone interpolation; unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Line Chart with Point Markers ](https://altair-viz.github.io/gallery/line_chart_with_points.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Line Chart with Stroked Point Markers ](https://altair-viz.github.io/gallery/line_chart_with_points_stroked.html) | `preprocessing_or_adapter` | non-default outlined point markers; unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Line Chart with Custom Order ](https://altair-viz.github.io/gallery/line_custom_order.html) | `native_family_candidate` | inline data required |
| [Line Chart with Percent axis ](https://altair-viz.github.io/gallery/line_percent.html) | `native_family_candidate` | inline data required |
| [Line Chart with Confidence Interval Band ](https://altair-viz.github.io/gallery/line_with_ci.html) | `preprocessing_or_adapter` | bootstrap confidence interval must be precomputed |
| [Line Chart with Last Value Labeled ](https://altair-viz.github.io/gallery/line_with_last_value_labeled.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Line Chart with Logarithmic Scale ](https://altair-viz.github.io/gallery/line_with_log_scale.html) | `native_family_candidate` | inline data required |
| [London Tube Lines ](https://altair-viz.github.io/gallery/london_tube.html) | `excluded_map` | geographic/geoshape intent |
| [Faceted County-Level Choropleth Maps ](https://altair-viz.github.io/gallery/maps_faceted_species.html) | `excluded_map` | geographic/geoshape intent |
| [Mosaic Chart with Labels ](https://altair-viz.github.io/gallery/mosaic_with_labels.html) | `compiler_gap` | mosaic layout and shared coordinates; unimplemented transform semantics: transform:calculate, transform:stack, transform:window |
| [Multiple Series Line Chart ](https://altair-viz.github.io/gallery/multi_series_line.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Multifeature Scatter Plot ](https://altair-viz.github.io/gallery/multifeature_scatter_plot.html) | `native_family_candidate` | inline data required |
| [Multi-Line Highlight ](https://altair-viz.github.io/gallery/multiline_highlight.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Multi-Line Tooltip ](https://altair-viz.github.io/gallery/multiline_tooltip.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Multi-Line Tooltip (Standard) ](https://altair-viz.github.io/gallery/multiline_tooltip_standard.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Multiple Interactions ](https://altair-viz.github.io/gallery/multiple_interactions.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Global Deaths from Natural Disasters ](https://altair-viz.github.io/gallery/natural_disasters.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Normalized Stacked Area Chart ](https://altair-viz.github.io/gallery/normalized_stacked_area_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Normalized Stacked Bar Chart ](https://altair-viz.github.io/gallery/normalized_stacked_bar_chart.html) | `native_family_candidate` | inline data required |
| [Normalized Parallel Coordinates ](https://altair-viz.github.io/gallery/normed_parallel_coordinates.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:joinaggregate, transform:window |
| [One Dot Per Zipcode ](https://altair-viz.github.io/gallery/one_dot_per_zipcode.html) | `excluded_map` | geographic/geoshape intent |
| [Pacman Chart ](https://altair-viz.github.io/gallery/pacman_chart.html) | `compiler_gap` | unsupported marks: mark:arc |
| [Parallel Coordinates ](https://altair-viz.github.io/gallery/parallel_coordinates.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Calculating Percentage of Total ](https://altair-viz.github.io/gallery/percentage_of_total.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:joinaggregate |
| [Pie Chart ](https://altair-viz.github.io/gallery/pie_chart.html) | `compiler_gap` | unsupported marks: mark:arc |
| [Pie Chart with Labels ](https://altair-viz.github.io/gallery/pie_chart_with_labels.html) | `compiler_gap` | unsupported marks: mark:arc |
| [Point map ](https://altair-viz.github.io/gallery/point_map.html) | `excluded_map` | geographic/geoshape intent |
| [Polar Bar Chart ](https://altair-viz.github.io/gallery/polar_bar_chart.html) | `compiler_gap` | unsupported marks: mark:arc |
| [Polynomial Fit Plot with Regression Transform ](https://altair-viz.github.io/gallery/poly_fit_regression.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:regression |
| [Pyramid Pie Chart ](https://altair-viz.github.io/gallery/pyramid.html) | `compiler_gap` | unsupported marks: mark:arc; non-Cartesian pie geometry |
| [Radial Chart ](https://altair-viz.github.io/gallery/radial_chart.html) | `compiler_gap` | unsupported marks: mark:arc |
| [Ranged Dot Plot ](https://altair-viz.github.io/gallery/ranged_dot_plot.html) | `preprocessing_or_adapter` | optional interactivity must be stripped; strip optional pan/zoom |
| [Ridgeline plot ](https://altair-viz.github.io/gallery/ridgeline_plot.html) | `compiler_gap` | density and vertical offsets; unimplemented transform semantics: transform:impute, transform:joinaggregate, transform:timeunit; bins may need explicit step/extent |
| [Faceted Scatter Plot ](https://altair-viz.github.io/gallery/scatter_faceted.html) | `native_family_candidate` | inline data required |
| [Scatter Plot with Href ](https://altair-viz.github.io/gallery/scatter_href.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Multi-panel Scatter Plot with Linked Brushing ](https://altair-viz.github.io/gallery/scatter_linked_brush.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Brushing Scatter Plot to Show Data on a Table ](https://altair-viz.github.io/gallery/scatter_linked_table.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Scatter Plot with Faceted Marginal Histograms ](https://altair-viz.github.io/gallery/scatter_marginal_hist.html) | `compiler_gap` | multiple coordinate-aligned axes |
| [Scatter Matrix ](https://altair-viz.github.io/gallery/scatter_matrix.html) | `preprocessing_or_adapter` | optional interactivity must be stripped; strip optional pan/zoom |
| [Scatter plot with point paths on hover and search box ](https://altair-viz.github.io/gallery/scatter_point_paths_hover.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Quantile-Quantile Plot ](https://altair-viz.github.io/gallery/scatter_qq.html) | `preprocessing_or_adapter` | quantile calculation; unimplemented transform semantics: transform:calculate, transform:quantile |
| [Simple Scatter Plot with Tooltips ](https://altair-viz.github.io/gallery/scatter_tooltips.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Scatter Plot and Histogram with Interval Selection ](https://altair-viz.github.io/gallery/scatter_with_histogram.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Simple Scatter Plot with Labels ](https://altair-viz.github.io/gallery/scatter_with_labels.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Interactive Scatter Plot and Linked Layered Histogram ](https://altair-viz.github.io/gallery/scatter_with_layered_histogram.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Scatter Plot with LOESS Lines ](https://altair-viz.github.io/gallery/scatter_with_loess.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:loess |
| [Scatter Plot with Minimap ](https://altair-viz.github.io/gallery/scatter_with_minimap.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Scatter Plot with Rolling Mean ](https://altair-viz.github.io/gallery/scatter_with_rolling_mean.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window, transform:timeunit; temporal fields/time units require materialization |
| [Scatter Plot with Shaded Area ](https://altair-viz.github.io/gallery/scatter_with_shaded_area.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Seattle Weather Interactive ](https://altair-viz.github.io/gallery/seattle_weather_interactive.html) | `native_family_candidate` | inline data required |
| [Selection Detail ](https://altair-viz.github.io/gallery/select_detail.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Using Selection Interval with mark_area ](https://altair-viz.github.io/gallery/select_mark_area.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Selection Histogram ](https://altair-viz.github.io/gallery/selection_histogram.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Interactive Average ](https://altair-viz.github.io/gallery/selection_layer_bar_month.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Selection zorder ](https://altair-viz.github.io/gallery/selection_zorder.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Simple Bar Chart ](https://altair-viz.github.io/gallery/simple_bar_chart.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Simple Heatmap ](https://altair-viz.github.io/gallery/simple_heatmap.html) | `preprocessing_or_adapter` | rect sizes/color encoding need spec verification |
| [Simple Histogram ](https://altair-viz.github.io/gallery/simple_histogram.html) | `native_family_candidate` | inline data required |
| [Simple Line Chart ](https://altair-viz.github.io/gallery/simple_line_chart.html) | `native_family_candidate` | matches documented native Cartesian plot family; exact example not run |
| [Simple Scatter Plot with Errorbars ](https://altair-viz.github.io/gallery/simple_scatter_with_errorbars.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate |
| [Simple Stacked Area Chart ](https://altair-viz.github.io/gallery/simple_stacked_area_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [Slider Cutoff ](https://altair-viz.github.io/gallery/slider_cutoff.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [Slope Graph ](https://altair-viz.github.io/gallery/slope_graph.html) | `native_family_candidate` | inline data required |
| [Sorted Error Bars showing Confidence Interval ](https://altair-viz.github.io/gallery/sorted_error_bars_with_ci.html) | `preprocessing_or_adapter` | bootstrap confidence interval must be precomputed |
| [Stacked Bar Chart ](https://altair-viz.github.io/gallery/stacked_bar_chart.html) | `native_family_candidate` | inline data required |
| [Stacked Bar Chart with Sorted Segments ](https://altair-viz.github.io/gallery/stacked_bar_chart_sorted_segments.html) | `native_family_candidate` | inline data required |
| [Stacked Bar Chart with Text Overlay ](https://altair-viz.github.io/gallery/stacked_bar_chart_with_text.html) | `native_family_candidate` | inline data required |
| [Stem and Leaf Plot ](https://altair-viz.github.io/gallery/stem_and_leaf.html) | `preprocessing_or_adapter` | text layout not yet tested |
| [Step Chart ](https://altair-viz.github.io/gallery/step_chart.html) | `native_family_candidate` | inline data required |
| [Streamgraph ](https://altair-viz.github.io/gallery/streamgraph.html) | `compiler_gap` | centered stacking |
| [Simple Strip Plot ](https://altair-viz.github.io/gallery/strip_plot.html) | `compiler_gap` | unsupported marks: mark:tick |
| [Strip Plot with Jitter ](https://altair-viz.github.io/gallery/strip_plot_jitter.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate |
| [Table Bubble Plot (Github Punch Card) ](https://altair-viz.github.io/gallery/table_bubble_plot_github.html) | `native_family_candidate` | inline data required |
| [Top K Items ](https://altair-viz.github.io/gallery/top_k_items.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Top K Letters ](https://altair-viz.github.io/gallery/top_k_letters.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Top-K Plot with Others ](https://altair-viz.github.io/gallery/top_k_with_others.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:window |
| [Line Chart with Varying Size ](https://altair-viz.github.io/gallery/trail_marker.html) | `compiler_gap` | unsupported marks: mark:trail; unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [The U.S. Employment Crash During the Great Recession ](https://altair-viz.github.io/gallery/us_employment.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:timeunit; temporal fields/time units require materialization |
| [US Income by State: Wrapped Facet ](https://altair-viz.github.io/gallery/us_incomebrackets_by_state_facet.html) | `excluded_map` | geographic/geoshape intent |
| [US Population by Age and Sex ](https://altair-viz.github.io/gallery/us_population_over_time.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [US Population: Wrapped Facet ](https://altair-viz.github.io/gallery/us_population_over_time_facet.html) | `native_family_candidate` | inline data required |
| [US Population Pyramid Over Time ](https://altair-viz.github.io/gallery/us_population_pyramid_over_time.html) | `excluded_interactive` | interaction is intrinsic to the gallery example |
| [U.S. State Capitals Overlaid on a Map of the U.S ](https://altair-viz.github.io/gallery/us_state_capitals.html) | `excluded_map` | geographic/geoshape intent |
| [Violin Plot ](https://altair-viz.github.io/gallery/violin_plot.html) | `preprocessing_or_adapter` | density computation is absent; unimplemented transform semantics: transform:density |
| [Waterfall Chart ](https://altair-viz.github.io/gallery/waterfall_chart.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate, transform:window |
| [Wheat and Wages ](https://altair-viz.github.io/gallery/wheat_wages.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:calculate |
| [Wilkinson Dot Plot ](https://altair-viz.github.io/gallery/wilkinson-dot-plot.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [Wind Vector Map ](https://altair-viz.github.io/gallery/wind_vector_map.html) | `excluded_map` | geographic/geoshape intent |
| [Window Rank Line Chart ](https://altair-viz.github.io/gallery/window_rank.html) | `preprocessing_or_adapter` | unimplemented transform semantics: transform:window |
| [World Map ](https://altair-viz.github.io/gallery/world_map.html) | `excluded_map` | geographic/geoshape intent |
| [World Projections ](https://altair-viz.github.io/gallery/world_projections.html) | `excluded_map` | geographic/geoshape intent |

## Implementation plan

1. Confirm native-family candidates by actually executing the original Altair chart with all datasets materialized.
2. Add normalizers for date/time units, window/rank/cumulative transforms, KDE/LOESS and precomputed uncertainty.
3. Add missing primitive families (`arc`, `tick`, `trail`, gradient/radial layouts and custom paths) only if explicitly in scope.
4. Store exact original-spec JSON, reference Altair SVG/PNG, emitted `.tex`, compiled `.pdf`, and a per-example numerical check. Only then change `exact_gallery_test` to `passed`.

## Reproduction

Source metadata: https://github.com/vega/altair/blob/main/_data/examples.json
Backend contract: https://github.com/marcusinthesky/vl-convert/blob/feature/experimental-pgfplots-export/docs/source/_topics/pgfplots.md
