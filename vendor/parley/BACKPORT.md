# Parley complex-script segmentation

This directory contains the published Parley 0.9.0 sources used by Bevy 0.19.1.
The original Apache-2.0 and MIT license files and source headers are retained.
The published source revision is recorded in `.cargo_vcs_info.json`.

Only the `complex-scripts` feature and its word/line segmenter selection have
been backported from the official
[Parley 0.10.0 implementation](https://github.com/linebender/parley/blob/v0.10.0/parley/src/analysis/mod.rs).
The feature is enabled only by the Windows native Screen adapter.
It selects ICU dictionary constructors instead of constructors without
complex-script models. It preserves the 0.9.0 public API and existing line-break options.

Bevy 0.19.1 and Parley 0.9.0 do not expose this feature. Enabling ICU data
features alone leaves the original constructor unchanged.
The upstream Bevy integration is documented in
[Bevy PR 25674](https://github.com/bevyengine/bevy/pull/25674).
Remove this backport when the selected stable Bevy dependency exposes the upstream feature.
