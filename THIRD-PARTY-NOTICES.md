# Third-Party Notices

This application uses the following third-party libraries and their respective licenses.

## Rust Dependencies

| Crate | License |
|-------|---------|
| serde | Apache-2.0 OR MIT |
| serde_json | Apache-2.0 OR MIT |
| rayon | Apache-2.0 OR MIT |
| hashbrown | Apache-2.0 OR MIT |
| memmap2 | Apache-2.0 OR MIT |
| zstd | MIT |
| clap | Apache-2.0 OR MIT |
| indicatif | MIT |
| egui | MIT |
| eframe | MIT |
| wgpu | Apache-2.0 OR MIT |
| image | Apache-2.0 OR MIT |
| ab_glyph | Apache-2.0 |
| rfd | MIT |

## Fonts

The desktop application bundles the **IBM Plex** typeface family, licensed under
the [SIL Open Font License 1.1](https://scripts.sil.org/OFL).

| Font | Usage | License |
|------|-------|---------|
| IBM Plex Sans (Regular) | UI text, and labels in exported PNG figures | SIL OFL 1.1 |
| IBM Plex Sans (SemiBold) | Headings and emphasis | SIL OFL 1.1 |
| IBM Plex Mono (Regular) | K-mer sequences and numeric columns | SIL OFL 1.1 |

Copyright © 2017–2018 IBM Corp. The full license text is distributed with the
sources at `gui/assets/fonts/OFL.txt`.

egui's built-in fonts are retained as a fallback for glyphs outside IBM Plex's
coverage (for example UI symbols), and remain under their original licenses.

## Scientific References

- DiMA methodology: Shan T, et al. (2024). "DiMA: Sequence Diversity Dynamics Analyser for Viruses." *NAR Genomics and Bioinformatics*. PMC11596295.
