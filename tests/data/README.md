# Test datasets

The alignments are stored bzip2-compressed (`bzip2 -9`; DiMA and the Python reference both read
`.bz2` directly, so the tests also exercise decompression); the one results file is
gzip-compressed, because DiMA reads results JSON plain, gzip or Zstandard, not bzip2. They were recompressed from the
earlier xz copies when DiMA stopped reading xz (D116); the decompressed bytes are unchanged.
They are committed byte-exact: `.gitattributes` marks this directory `-text`. Checksums are
SHA-256 of the **decompressed** bytes.

| File | Sequences × columns | SHA-256 (decompressed) | Bytes |
|---|---|---|---|
| `../../samples/mers_spike_aa.fasta` (MERS-CoV spike protein; the paper's Fig. 1C dataset) | 191 × 1,353 | `5543ec7b28d1601aae3bd22f8cbad45c819d3575a899ea54e401c5ac1cf5b97d` | 276,539 |
| `MERS_Spike_nt.fasta.bz2` (MERS-CoV spike gene, lower-case) | 499 × 4,065 | `b1d1880e2546504b57d93e7b9343ff80edfb312e19886220d5e2f1a437253243` | 2,120,534 |
| `HIV1_ALL_2020_pol_PRO.fasta.bz2` (HIV-1 pol protein, LANL alignment, CRLF) | 3,756 × 1,197 | `281f1fcd266d00865b910f7467dddf63fe4acd86e1e9c5c53596fe4e4d154af4` | 4,754,950 |
| `HIV1_FLT_2022_pol_DNA.fasta.bz2` (HIV-1 group M pol gene; the paper's HIV dataset) | 2,637 × 3,390 | `45878c9e55703c148105139332346a8b3152bac0ac85d26cb122c2a9172895a2` | 9,194,803 |
| `tobrfv_tm2.fasta.bz2` (ToBRFV 5'-120nts reads, tm-2 plants; Wu 2025) | 10,833 × 120 | `2665bb5348c260475aa66008cb8c79e674b465c41b82720acf88fb201c88ffd4` | 1,427,535 |
| `tobrfv_tm22.fasta.bz2` (ToBRFV 5'-120nts reads, Tm-2² plants; Wu 2025) | 6,969 × 120 | `d44eb59de3f4697a335d0b567dbfe6611bd4d8b6aec108c43fa03a248cda424d` | 917,725 |
| `results-schema-1.0.0.json.gz` (results written by dima 1.0.0, schema 1.0.0) | — | `4ee511590ca0fb53f95126e078ee155693f7264ab7f1d232c7870adc365faff9` | 15,400 |

The MERS amino-acid dataset is byte-identical to `samples/mers_spike_aa.fasta` and is not
duplicated here.

## Provenance and licences

* **MERS-CoV spike (protein and nucleotide).** Sequences and metadata from NCBI Virus / GenBank
  (headers `Accession|Host|Geolocation|Year`). NCBI places no restrictions on the use of these
  data; see the NCBI policies page. Cite: Hatcher EL, et al. Virus Variation Resource. *Nucleic
  Acids Res* 2017;45:D482–D490, doi:10.1093/nar/gkw1065. The protein alignment is the dataset of
  Tharanga et al. 2025, Fig. 1C.
* **HIV-1 pol (protein and nucleotide).** Pre-curated alignments from the Los Alamos HIV Sequence
  Database (RRID:SCR_000614), HIV Sequence Compendium (filtered "FLT" 2022 pol DNA; "ALL" 2020 pol
  protein). The DNA alignment is the dataset analysed in Tharanga et al. 2025 (2,637 sequences,
  3,364 27-mer positions, 564 LS, 680 NS). The protein alignment contains the LANL symbols `*`
  (stop, 3,625 times) and `#` (frameshift, 192 times). Cite the Los Alamos HIV Sequence Database and the
  HIV Sequence Compendium as requested on <https://www.hiv.lanl.gov>.
* All four files were supplied by the DiMA maintainer (retrieved 2026-10-04) for validation.
* **ToBRFV (tm-2 and Tm-2²).** From the supplementary data of Wu J. Similarity-weighted entropy
  for quantifying genetic diversity in viral quasispecies. *Virus Evol* 2025;11(1):veaf029,
  doi:10.1093/ve/veaf029, published under the Creative Commons Attribution 4.0 licence (CC BY
  4.0): Supplementary File 2 lists the unique 120-nt sequences of the 5'-120nts region of tomato
  brown rugose fruit virus in tm-2 and Tm-2² tomato plants with their read counts (48 sequences
  and 10,833 reads; 47 and 6,969; sequences below 0.1% of the reads were excluded by the
  author). Each unique sequence is written once per read, so that a run at $k = 120$ (one
  position) counts the reads; records are named `SeqN_r`. Retrieved from Europe PMC (PMC12079039,
  supplementary files) on 2026-11-25 and converted with
  `python3 scripts/reference/tobrfv_from_supplement.py "Supplementary File 2.xlsx" DIR`,
  then `bzip2 -9`. Used for the similarity-weighted entropy paper test.
* **`results-schema-1.0.0.json.gz`.** The golden results of the `tiny` test input
  (`tests/golden/tiny/results.json`) as dima 1.0.0 wrote them, before schema 1.1.0, frozen to
  test that older results are read as runs without the options their schema did not have
  (`gzip -9 -n`).

## `dima-v5.0.9/` — reference implementation fixtures

Copied from the original DiMA repository (<https://github.com/BVU-BILSAB/dima>, tag `v5.0.9`,
commit `2edeae2173b366475b8acc05bf422bccf63a42ca`, `tests/data/`), MIT licence (declared in its
`pyproject.toml` and `Cargo.toml`). Used for cross-checks against the published implementation
(v5.0.9 stores incidences as `f32`, so comparisons use ~1e-5 relative tolerance; its entropy
correction is randomised, so entropies are compared statistically, not exactly).

| File | SHA-256 (decompressed) | Bytes |
|---|---|---|
| `input.afa` | `b9bb421a4bb99ff0d7224693fa17a1dc7b17adfff9e1731cd7bf11ca9b6a944f` | 36,160 |
| `input_incons_headers.afa` | `02c48f5e968a8c0f939e06bb97cb5937e997e3404c02048f186ef204131e4dd9` | 128,075 |
| `basic_output.json` | `7a42ded7b980c3a6ef5fda20a9d50d1899784c6934b7a1cf6a8efa2a4d8d41a8` | 608,075 |
| `advance_output.json` | `8cff52355dca3870dee9b45c8f7f2fbaf717131df715640841eafa553bc83251` | 2,064,189 |
| `incons_headers_output.json` | `b25902e3180d031d2c4323ffa03dd0150cf54780f42280d2b6999ba466220952` | 5,836,561 |
| `incons_headers_fillna_output.json` | `7d9c7156b423908236395f3f394ea7c550ddb0f37d7aeceff9720a066f3610ed` | 5,829,033 |
