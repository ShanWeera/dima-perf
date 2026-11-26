# Test datasets

The alignments are stored bzip2-compressed (`bzip2 -9`; DiMA and the Python reference both read
`.bz2` directly, so the tests also exercise decompression); the one results file is
gzip-compressed, because DiMA reads results JSON plain, gzip or Zstandard, not bzip2. The four
original alignments were recompressed from the earlier xz copies when DiMA stopped reading xz
(D116); the decompressed bytes are unchanged.
They are committed byte-exact: `.gitattributes` marks this directory `-text`. Checksums are
SHA-256 of the **decompressed** bytes.

| File | Sequences × columns | SHA-256 (decompressed) | Bytes |
|---|---|---|---|
| `../../samples/mers_spike_aa.fasta` (MERS-CoV spike protein; the paper's Fig. 1C dataset) | 191 × 1,353 | `5543ec7b28d1601aae3bd22f8cbad45c819d3575a899ea54e401c5ac1cf5b97d` | 276,539 |
| `MERS_Spike_nt.fasta.bz2` (MERS-CoV spike gene, lower-case) | 499 × 4,065 | `b1d1880e2546504b57d93e7b9343ff80edfb312e19886220d5e2f1a437253243` | 2,120,534 |
| `HIV1_ALL_2020_pol_PRO.fasta.bz2` (HIV-1 pol protein, LANL alignment, CRLF) | 3,756 × 1,197 | `281f1fcd266d00865b910f7467dddf63fe4acd86e1e9c5c53596fe4e4d154af4` | 4,754,950 |
| `HIV1_FLT_2022_pol_DNA.fasta.bz2` (HIV-1 group M pol gene; the paper's HIV dataset) | 2,637 × 3,390 | `45878c9e55703c148105139332346a8b3152bac0ac85d26cb122c2a9172895a2` | 9,194,803 |
| `tobrfv_tm2.fasta.bz2` (ToBRFV 5'-120nts reads, tm-2 sample; Wu 2025) | 10,833 × 120 | `68a7158e652d1b8ccea4f5299c32f1935d187cb8f524a946d95bc5f8b63fdd30` | 1,376,295 |
| `tobrfv_tm22.fasta.bz2` (ToBRFV 5'-120nts reads, Tm-2² sample; Wu 2025) | 6,969 × 120 | `ff945575be50fe26a13cffe0094e73eb113bac76336fc3c325e4d6555d55e326` | 885,401 |
| `pstvd_loop27_pool.fasta.bz2` (PSTVd loop 27, mutant pool; Wu et al. 2024) | 73,627 × 6 | `b09fc4f58eab692bfbde7cd163f83f66b265cb8413034e6c3c5c45d6720a7b5c` | 1,049,087 |
| `pstvd_loop27_ir1.fasta.bz2` (PSTVd loop 27, inoculated region, plant 1; Wu et al. 2024) | 56,701 × 6 | `425c87192dfbcd86777345555fb4b7262cddb987770bd7ec66efa74f5bc14b54` | 787,303 |
| `pstvd_loop27_ir2.fasta.bz2` (PSTVd loop 27, inoculated region, plant 2; Wu et al. 2024) | 61,108 × 6 | `e62fad37acb5850ae81bdc70bb246bb74215104dbbbf31c65e324f32aa7c5907` | 852,243 |
| `pstvd_loop27_ir3.fasta.bz2` (PSTVd loop 27, inoculated region, plant 3; Wu et al. 2024) | 52,903 × 6 | `695ff41c54b0a08fa9dbd54d608e5a3dc9a9e9db40168c9c8ac5952b96fa720e` | 735,604 |
| `pstvd_loop27_lm1.fasta.bz2` (PSTVd loop 27, leaf margin, plant 1; Wu et al. 2024) | 30,013 × 6 | `a4cdb737b385d28f8add88a0fb3b32183e9e707f628a3183f6482cdd51a38c1f` | 394,454 |
| `pstvd_loop27_lm2.fasta.bz2` (PSTVd loop 27, leaf margin, plant 2; Wu et al. 2024) | 33,075 × 6 | `65256ca3d9d3bce31ab6892a1639a81ddd2ad105586939da897c196b928a45e9` | 432,924 |
| `pstvd_loop27_lm3.fasta.bz2` (PSTVd loop 27, leaf margin, plant 3; Wu et al. 2024) | 38,708 × 6 | `9e425ef037cf819cb917ca3ec985d92b2c99a86ee9326f39dd43908ec1b438f1` | 505,928 |
| `pstvd_loop27_sys1.fasta.bz2` (PSTVd loop 27, systemic leaves, plant 1; Wu et al. 2024) | 62,182 × 6 | `aed884a05d2b3bf551b30ab335afc32eada35c7ca8f9d3f656c495a3d817df13` | 809,367 |
| `pstvd_loop27_sys2.fasta.bz2` (PSTVd loop 27, systemic leaves, plant 2; Wu et al. 2024) | 65,426 × 6 | `fe0f5c07d99c9baa1563e522f881f18b031655f24598373347208349dae75594` | 852,651 |
| `pstvd_loop27_sys3.fasta.bz2` (PSTVd loop 27, systemic leaves, plant 3; Wu et al. 2024) | 50,835 × 6 | `d1a144861b35a6327c8b9a7187def77453c099a5031e46d3203f68dd488b4568` | 662,522 |
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
  doi:10.1093/ve/veaf029; © The Author(s) 2025, published by Oxford University Press under the
  Creative Commons Attribution 4.0 licence (CC BY 4.0,
  <https://creativecommons.org/licenses/by/4.0/>); converted as described below, not otherwise
  changed. Supplementary File 2 lists the unique 120-nt sequences of the 5'-120nts region of
  tomato brown rugose fruit virus in the tm-2 and Tm-2² samples with their read counts (48
  sequences and 10,833 reads; 47 and 6,969). Wu reports excluding sequences below 0.1% of the
  reads; the tm-2 sheet nevertheless lists eight with 10 reads (0.092%). All are kept: with all
  48, the tm-2 values of Wu's Fig. 6d are reproduced. Each unique sequence is written once per
  read, so that a run at $k = 120$ (one position) counts the reads; every record of a sequence
  carries the sheet's ID (`SeqN`). Retrieved from Europe PMC (PMC12079039,
  supplementary files) on 2026-11-25 (`Supplementary File 2.xlsx`, SHA-256
  `4d3e3fb6c0f59925ff56008c795d51b8823f26bd32a9dc5b12d780496190b074`) and converted with
  `python3 scripts/reference/tobrfv_from_supplement.py "Supplementary File 2.xlsx" DIR`,
  then `bzip2 -9`. Used for the similarity-weighted entropy paper test.
* **PSTVd loop 27 (mutant pool and nine plant libraries).** From the supporting information of
  Wu J, Zhang Y, Nie Y, Yan F, Zirbel CL, Bisaro DM. RNA three-dimensional structure drives the
  sequence organization of potato spindle tuber viroid quasispecies. *PLoS Pathog*
  2024;20(4):e1012142, doi:10.1371/journal.ppat.1012142; © 2024 Wu et al., an open access article
  distributed under the Creative Commons Attribution License (CC BY 4.0,
  <https://creativecommons.org/licenses/by/4.0/>); converted as described below, not otherwise
  changed. Sheet "Loop27" of its S2 Table lists the unique sequences of PSTVd loop 27 (wild type
  UUUUCA, mutated to UNNNNA; written in DNA letters) with their read counts, for the mutant pool
  and for the inoculated region (IR), the margin of the inoculated leaves (LM) and the systemic
  leaves (Sys) of three *Nicotiana benthamiana* plants each: the data of Wu 2025's Fig. 5c. The
  sheet's "reads per thousand" are relative to each library's total in the study's S1 Table; the
  listed reads add up to between 3 fewer and 2 more (73,627 of 73,628 for the pool), and IR-1,
  IR-2 and Sys-2 list 1 more, 3 more and 1 fewer unique sequences than S1 Table counts. The
  listed sequences and reads are used as they are. Each unique sequence is written once per read,
  so that a run at $k = 6$ (one position) counts the reads; every record of a sequence is named
  `SeqN`, N being its place in the library's list. Retrieved from Europe PMC (PMC11020406,
  supplementary files) on 2026-11-25 (`journal.ppat.1012142.s002.xlsx`, SHA-256
  `b1312329cd06ce56c2cbeaf61923d552658ab6db3ae6b7663b94cf159fe7388f`) and converted with
  `python3 scripts/reference/pstvd_from_supplement.py journal.ppat.1012142.s002.xlsx DIR`, which
  checks every row, then `bzip2 -9`. Used for the similarity-weighted entropy paper test.
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
