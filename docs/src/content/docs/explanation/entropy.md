---
title: Entropy and its correction
description: Shannon entropy of k-mer variants, the paper's sample-size correction (Fig. 2) and why DiMA computes its exact expectation, the alternative estimators, sample coverage, why positions without support have no entropy, and the optional similarity-weighted entropy (Wu 2025).
---

At every position DiMA summarises the distribution of variants by its Shannon entropy
[@shannon1948]: 0 bits when every sequence carries the same k-mer, more as variants become more
numerous and more even. This page derives the observed entropy, the paper's correction for small
samples [@tharanga2025], the way DiMA evaluates that correction exactly, the optional
alternative estimators, the sample coverage reported next to them, and the optional
[similarity-weighted entropy](#similarity-weighted-entropy-opt-in).

Notation: a position has support $N$ (sequences with a valid k-mer) and $D$ distinct variants with
counts $c_1, \dots, c_D$; $f_c$ is the number of variants seen exactly $c$ times (so $f_1$ counts
singletons and $f_2$ doubletons); $T$ is the support threshold.

## Observed (plug-in) entropy

With the observed frequencies $\hat p_i = c_i / N$, the plug-in entropy in bits is

$$
\begin{equation}
\hat H = -\sum_{i=1}^{D} \hat p_i \log_2 \hat p_i
       = -\sum_{c} f_c \, \frac{c}{N} \log_2 \frac{c}{N} .
\label{eq:plugin}
\end{equation}
$$

It is reported for every position with support as `observed_entropy`. It is biased downwards:
variants that are present in the population but absent from the sample contribute nothing, and to
first order the expected value falls short of the true entropy by about $(D-1)/(2N\ln 2)$ bits
[@miller1955; @paninski2003]. The bias grows with diversity and shrinks with support, so
positions with different support are not directly comparable without a correction.

## The paper's correction (Fig. 2)

DiMA's correction [@tharanga2025, Fig. 2] belongs to the family of extrapolation methods used in
information estimation: compute the entropy of subsamples of decreasing size and extrapolate to
infinite sample size, where $1/n \to 0$ [@strong1998; @panzeri2007]. For a position with
$N > T$:

1. **Grid of subsample percentages.** Let

   $$
   \begin{equation}
   x = \left\lceil \frac{100\,T}{N} \right\rceil .
   \label{eq:start}
   \end{equation}
   $$

   The grid starts at $R_0 = 50$ if $x < 50$, otherwise at $x$ rounded up to a multiple of 5,
   and runs $R \in \{R_0, R_0 + 5, \dots, 95\}$, plus 100%. Fig. 2 gives the grid as
   $\{x\%, \dots, 90, 95, 100\%\}$, or $\{50\%, \dots, 90, 95, 100\%\}$ when $x \le 50$; the
   step of 5 throughout and the rounding of $x$ up to a multiple of 5 are those of the authors'
   implementation, DiMA v5.0.9 (`calculate_entropy` in `src/lib.rs`).
2. **Subsample sizes.** For each $R < 100$, the subsample size is

   $$
   \begin{equation}
   n_R = \left\lceil \frac{R \, N}{100} \right\rceil .
   \label{eq:size}
   \end{equation}
   $$

3. **Subsample entropies.** The paper draws one subsample of $n_R$ k-mers *with replacement* from
   the position's k-mers and computes its plug-in entropy $\hat H_{n_R}$. The 100% point is the
   full data set itself: $(1/N, \hat H)$, not resampled.
4. **Regression.** The points $(1/n_R, \hat H_{n_R})$ are fitted by ordinary least squares,
   $\hat H_n \approx \beta_0 + \beta_1 / n$, and the **intercept** $\beta_0$ — the entropy
   extrapolated to $n \to \infty$ — is the corrected entropy.
5. **Fallbacks.** The observed entropy $\hat H$ is reported instead when $N \le T$, when there are
   fewer than five points, when all subsample sizes are equal (no line is defined), or when the
   intercept is negative (which never happens to the exact expectation DiMA computes; see
   [the negative-intercept rule](#the-negative-intercept-rule)).

Rounding $x$ up in $\eqref{eq:start}$ guarantees that every subsample has at least $T$ k-mers:
$R \ge x \ge 100T/N$, so $n_R \ge RN/100 \ge T$. Rounding it down would not: for $T = 100$, ten
values of $N$ between 110 and 198 would give subsamples of 99. The ceiling is what the original
implementation adopted in its commit "Fixed algorithm after discussion with Dr. Asif" (February
2022) and the glyph printed in Fig. 2, although the figure's note beside it, "Lower-bound or
minorant, nearest whole number", reads like rounding down; the subsample sizes $\eqref{eq:size}$ are rounded up too
(Fig. 2, "Upperbound"). DiMA computes $\eqref{eq:start}$ in exact integer arithmetic. DiMA v5.0.9
computed it in floating point, where for example $T = 55$, $N = 100$ gives 56 instead of 55; for
the usual thresholds $T = 30$ and $T = 100$ the two agree for every $N$ up to two million (the
range checked).

With fewer than five points the correction is not applied — a minimum the paper does not state,
taken from DiMA v5.0.9 — which happens when $x > 80$, that is for $N$ below about $1.25\,T$ (for
$T = 100$: $N = 101$ to 124). Duplicate subsample sizes are kept, as in DiMA v5.0.9 (the paper
does not say).

**Example.** At MERS-CoV spike position 25 ($N = 170$) with $T = 100$: $x = \lceil 58.8 \rceil =
59$, so the grid is 60, 65, …, 95 and 100%, nine points, with subsample sizes 102, 111, 119, 128,
136, 145, 153, 162 and 170. With $T = 30$: $x = 18 < 50$, so the grid starts at 50: eleven
points, from 85 to 170 k-mers.

## The exact expectation

The paper's procedure is random: run twice, it gives two different answers, because each
$\hat H_{n_R}$ comes from one random subsample. DiMA reports the **expected value** of the
procedure instead, which it computes exactly. This changes no part of the method; it removes the
noise of one random draw (the paper's negative-intercept rule, applied to that expected value,
then never applies: see [below](#the-negative-intercept-rule)).

**The intercept is linear in the responses.** For fixed design points $u_j = 1/n_j$ (they depend
only on $N$ and $T$), the least-squares intercept is a weighted sum of the responses,

$$
\begin{equation}
\hat\beta_0 = \sum_j w_j \, y_j , \qquad
w_j = \frac{1}{m} - \bar u \, \frac{u_j - \bar u}{\sum_l (u_l - \bar u)^2} ,
\label{eq:ols}
\end{equation}
$$

with $m$ points and mean design point $\bar u$. Its expectation over the random subsamples is
therefore the intercept of the line fitted to the *expected* subsample entropies,
$\operatorname{E}[\hat\beta_0] = \sum_j w_j \operatorname{E}[y_j]$.

**The expected subsample entropy has a closed form.** A with-replacement sample of size $n$ from
the observed k-mers is a draw from the empirical distribution — a nonparametric bootstrap sample
[@efron1979] — so its counts are multinomial, $(X_1,\dots,X_D) \sim
\operatorname{Multinomial}(n; \hat p_1, \dots, \hat p_D)$, and each count is binomial,
$X_i \sim \operatorname{Binomial}(n, \hat p_i)$. The plug-in entropy is a sum of one term per
variant, so by linearity of expectation

$$
\begin{equation}
\operatorname{E}\bigl[\hat H_n\bigr]
  = \sum_{c} f_c \sum_{x=1}^{n} \binom{n}{x} p_c^{\,x} (1-p_c)^{\,n-x}
    \left(-\frac{x}{n}\log_2\frac{x}{n}\right),
\qquad p_c = \frac{c}{N} .
\label{eq:expected}
\end{equation}
$$

Variants with the same count contribute equally, which is why only the counts of counts $f_c$
are needed. DiMA evaluates each binomial sum by starting at its mode and walking outwards with
the ratio of consecutive probabilities, $\frac{n-x}{x+1}\cdot\frac{p}{1-p}$, stopping when a term
falls below $10^{-18}$ of the running total. No factorials or powers are formed, nothing
overflows, and the cost is proportional to $\sqrt{n\,p(1-p)}$ per distinct count. The 100% point
stays $(1/N, \hat H)$, and the fallbacks of the paper apply to the expected intercept.

**Same mean, no larger variance.** The exact estimator is the conditional expectation, given
the data, of the intercept the paper's procedure fits from one random draw per grid point: it
averages out the resampling noise and nothing else. By the Rao–Blackwell theorem it has the same
expected value as that random intercept and a variance that is never larger
[@rao1945; @blackwell1947]. It is also what the original method gives when many random subsets
are averaged before extrapolating, as DiMA's predecessors did: Khan et al. regressed the
entropies of repeatedly drawn random sub-alignments on $1/N$ [@khan2008], with averaging
over several random subsets the $1/N$ relation held for sets as small as 20 sequences
[@miotto2008], and
the AVANA tool extrapolated from 100 random sub-alignments by default [@khan2017].

**Consequences.**

- Results are reproducible bit for bit; no random seed is involved.
- On the paper's HIV-1 *pol* dataset the mean corrected entropy is 5.3504 bits; the paper reports
  about 5.35, <!-- docs-numbers:hiv-run-mean:start -->
  and the mean intercept of one run of the random procedure, before its negative-intercept rule, has
  a standard deviation of 0.0020 bits around it<!-- docs-numbers:hiv-run-mean:end --> (see
  [Comparison with the papers](/explanation/paper-comparison/)).
- **Fig. 1C.** At MERS-CoV spike position 25 the paper shows 1.38 bits ($T = 30$). <!-- docs-numbers:fig1c:start -->
  The exact expectation is 1.4451 bits, and the intercept of one run of the random procedure has a
  standard deviation of 0.2241 bits there, so 1.38 lies 0.29 standard deviations below the
  expectation: an ordinary draw of the random procedure, not a different method. With $T = 100$ the
  expectation is 1.4617 bits and the standard deviation 0.3047 bits.<!-- docs-numbers:fig1c:end -->

These standard deviations are exact too: the subsamples are drawn independently, so the variance
of the random intercept is $\sum_j w_j^2 \operatorname{Var}(\hat H_{n_j})$, and each variance
follows from the multinomial distribution of the subsample counts.

### The negative-intercept rule

The paper reports the observed entropy when the regression "may potentially result in a negative
entropy value due to small number of resamples" [@tharanga2025]. DiMA applies this rule to the
expected intercept, and that intercept is never negative, so the rule never applies:

1. The expected entropy of a subsample never decreases with its size $n$: the frequencies of
   $n + 1$ draws are the average of the frequencies of their $n + 1$ subsets of $n$ draws, each
   distributed as $n$ draws, and entropy is concave.
2. It never exceeds the observed entropy $\hat H$: a subsample drawn with replacement has the
   observed frequencies as its expected frequencies, and entropy is concave (Jensen's
   inequality).
3. So the points never rise as $1/n$ grows, the least-squares slope is at most 0, and the
   intercept $\beta_0 = \bar y - \beta_1 \bar u$ (with $\bar u$ the mean of $1/n$) is at least
   the mean of the points $\bar y$, which is at least 0. It is 0 only at a fully conserved
   position.

DiMA therefore has no fallback for a negative intercept. Instead, every reported entropy, from
every estimator, is checked to be a finite number of at least 0 bits; a value that is not could
only come from a bug, and it stops the run as an internal error (exit 1) instead of being
replaced by another value [@wilson2014, Practice 5a; @hunterzinck2021, Rule 5]. A property test
confirms the three steps above on random and adversarial count profiles.

DiMA v5.0.9 applies the rule to each random run instead, and rounds the subsample sizes down (see
[Differences from DiMA v5.0.9](/explanation/differences/#method)). Where one or a few sequences
carry a variant other than the [Index](/reference/glossary/#index-major-minor-and-unique), a
single run's intercept is often negative and is then replaced by the larger observed entropy, so
v5.0.9's average over many runs is higher there. The
table gives the share of runs that fall back and how far they raise the average above DiMA's
value, at positions where every other sequence carries the Index:

<!-- docs-numbers:v509-fallbacks:start -->

| Sequences that differ from the Index | Runs that fall back | Average raised by | DiMA's value |
|---|---|---|---|
| one | 20.2–21.6% | 34.7–39.2% | 0.0044–0.0469 bits |
| two, both carrying the same k-mer | 12.7–14.4% | 17.9–21.6% | 0.0082–0.0834 bits |
| two, carrying two different k-mers | 12–14% | 18–21% | 0.0089–0.0937 bits |
| five, all carrying the same k-mer | 3.0–4.7% | 3.5–5.9% | 0.0180–0.1715 bits |
| five, all carrying different k-mers | 3–5% | 4–6% | 0.0221–0.2341 bits |

Each range is over $N$ = 200, 300, 500, 1,000, 2,000 and 3,000 sequences ($T = 100$), where the grid
has all eleven points and the subsample sizes need no rounding, so v5.0.9's runs differ from the
paper's procedure only by the rule. Exact where the differing sequences carry one k-mer; seeded
Monte Carlo of 100,000 runs per $N$ where they carry different ones (95% intervals within ±0.4
percentage points, so those rows give whole percentages).

<!-- docs-numbers:v509-fallbacks:end -->

With fewer points ($N$ close to $T$) the effect is larger: <!-- docs-numbers:v509-few-points:start -->
one sequence differing raises the average by 50% at $N = 171$ and by 105% at $N = 130$ ($T = 100$;
exact)<!-- docs-numbers:v509-few-points:end -->.
On the MERS-CoV spike protein ($T = 100$) the eligible mean is 0.1261 bits here and <!-- docs-numbers:v509-means:start -->
0.1417 bits averaged over v5.0.9 runs (seeded Monte Carlo of 2,000 runs, 95% interval ±0.0001); on
the HIV-1 *pol* datasets the two differ by less than 0.00026 bits (DNA) and 0.0011 bits (protein),
under half the standard deviation of the mean of a single run (0.0020 and 0.0027 bits)<!-- docs-numbers:v509-means:end -->.

<!-- docs-numbers:population-study:start -->

In samples of $N$ = 171, 200, 300, 500, 1,000, 2,000 and 3,000 sequences from two-variant
populations with a known entropy (minor-variant frequency 0.05, 0.1, 0.2, 0.5, 1, 2 and 5%; $T$ = 30
and 100), v5.0.9's per-run rule overestimated the entropy by up to 31% on average with $T = 100$ and
20% with $T = 30$ (seeded Monte Carlo of 100,000 runs per case, 95% intervals within ±0.5 percentage
points). The exact expectation underestimated it by up to 27% when less than one minor-variant
sequence was expected and by up to 6.4% otherwise (and overestimated it by at most 0.14%), within
3.3 percentage points of the error of the Miller–Madow correction. Neither v5.0.9's per-run rule nor
the exact expectation had the smaller error everywhere.

<!-- docs-numbers:population-study:end -->

## Alternative estimators

`--estimator` selects another estimator for the reported `entropy`. The paper's correction
(`dima`) is the default; the alternatives apply at every position with $N \ge 2$, whatever $T$ is,
while the summaries still cover eligible positions only. Each position records the method actually
used (`entropy_method`) and, when the requested one could not be used, why (`entropy_fallback`).

**Miller–Madow** (`miller-madow`) adds the first-order bias term [@miller1955]:

$$
\begin{equation}
\hat H_\text{MM} = \hat H + \frac{D - 1}{2 N \ln 2} .
\label{eq:mm}
\end{equation}
$$

**Chao–Wang–Jost** (`chao-wang-jost`) estimates the entropy from the rate at which new variants
are discovered as the sample grows; it was the least biased estimator in its authors' comparison
[@chao2013]. In nats,

$$
\begin{equation}
\hat H_\text{CWJ} = \sum_{c=1}^{N-1} f_c \, \frac{c}{N} \sum_{k=c}^{N-1} \frac{1}{k}
  + \frac{f_1}{N} (1-A)^{1-N} \left( -\ln A - \sum_{r=1}^{N-1} \frac{(1-A)^r}{r} \right),
\label{eq:cwj}
\end{equation}
$$

divided by $\ln 2$ for bits, with

$$
\begin{equation}
A = \begin{cases}
  \dfrac{2 f_2}{(N-1) f_1 + 2 f_2} & f_2 > 0, \\[1.5ex]
  \dfrac{2}{(N-1)(f_1 - 1) + 2} & f_2 = 0,\; f_1 > 0, \\[1.5ex]
  1 & f_1 = f_2 = 0 .
\end{cases}
\label{eq:cwj-a}
\end{equation}
$$

DiMA evaluates the inner sums with exact harmonic numbers for small counts and an asymptotic series
for large ones. The second term is computed exactly, in a time independent of $N$, through an
identity with a hypergeometric function, so that $(1-A)^{1-N}$ is never formed and nothing
overflows or cancels; [Numerical accuracy](/explanation/numerical-accuracy/#chaowangjosts-second-term)
describes the method, its sources and its measured accuracy. When $A = 1$ the second term is 0.

**Raw** (`raw`) reports the observed entropy $\eqref{eq:plugin}$ unchanged.

Both alternatives fall back to the observed entropy when $N = 1$ (`single_observation`), where
the observed entropy is 0. Unlike the paper's estimator, they are applied at every position with
$N \ge 2$, LS and ELS positions included; the support classes then only decide which positions
enter the summaries.

| Estimator | MERS-CoV spike position 25 |
|---|---|
| observed ($\hat H$) | 1.4224 bits |
| `dima`, $T = 100$ | 1.4617 bits |
| `dima`, $T = 30$ | 1.4451 bits |
| `miller-madow` | 1.4606 bits |
| `chao-wang-jost` | 1.4901 bits |

## Sample coverage

Next to every entropy, DiMA reports the estimated **sample coverage** $\hat C$: the probability
that one more sequence would carry a k-mer already seen at that position. It goes back to Turing's
estimate $1 - f_1/N$ [@good1953]; DiMA uses the improved estimator of Chao and Jost
[@chao2012, Eq. 4a]:

$$
\begin{equation}
\hat C = 1 - \frac{f_1}{N} \cdot \frac{(N-1) f_1}{(N-1) f_1 + 2 f_2} ,
\label{eq:coverage}
\end{equation}
$$

with $(N-1)(f_1-1)/((N-1)(f_1-1)+2)$ as the second factor when $f_2 = 0$ (the same convention as
$A$ in $\eqref{eq:cwj-a}$, so that $\hat C = 1 - (f_1/N)(1-A)$), and $\hat C = 1$ when $f_1 = 0$.
It is undefined (`null`) for $N < 2$. Low coverage means that much of the diversity at that
position has probably not been sampled: its entropy, corrected or not, rests on an incomplete
picture. `--min-coverage X` flags positions with $\hat C < X$ (`below_min_coverage`) and adds a
warning; coverage is always reported. At MERS-CoV spike position 25, $f_1 = 4$ and $f_2 = 0$, so
$\hat C = 1 - \frac{4}{170}\cdot\frac{507}{509} = 0.9766$.

## Positions without support

A position where no sequence has a valid k-mer (NS) has no variants and therefore no
distribution: its entropy is not 0 bits, it is undefined. The paper assigns such positions a
"pseudo entropy" of 0, tagged NS so that it is not mistaken for conservation, and leaves them out
of its summaries (Fig. 2; Methods) [@tharanga2025]. DiMA departs from the paper here,
deliberately: it keeps the NS tag but writes `null` in JSON and `NA` in the tables, with
`entropy_method` `not_estimable`, and also leaves NS positions out of the summary statistics.
This follows the treatment of missing data in statistics and in the field: a value
that was not observed is recorded as missing, not as zero [@rubin1976; @schafer2002]; in sequence
count data, zeros that arise from absent sampling are not the same as true zeros
[@silverman2020; @lazar2016]; conservation scores leave columns dominated by gaps unscored rather
than calling them conserved [@capra2007]; and tidy data use an explicit missing-value code
[@broman2018].

A 0 in the output would also be averaged as a real value by any later summary that does not
filter on the tag, biasing it towards conservation. On the paper's HIV-1 *pol* dataset (680 NS
positions) the mean over all positions with NS counted as 0 — DiMA v5.0.9's rule, still reported
as `all_positions_summary` for comparison — is 3.443 bits, against 5.350 bits over the eligible
positions.

## Similarity-weighted entropy (opt-in)

Shannon entropy treats two variants that differ at one residue exactly like two unrelated ones.
Similarity-weighted entropy [@wu2025, Section 2.1 and Supplementary File 1] weights each
variant's entropy term by how *dissimilar* it is to the rest of the position, so near-identical
variants count for less. It is not part of the DiMA method and is computed only on request:
`dima analyze --similarity-weighted-entropy`, or **Compute similarity-weighted entropy (Wu 2025)**
under **Advanced** in the [desktop app](/how-to/desktop-app/#similarity-weighted-entropy).
Without it, the three values below are `null` (`NA` in the positions table); with it, nothing else
changes.

At a position with support $N$, let variant $i$ (a distinct k-mer, the
[Index](/reference/glossary/#index-major-minor-and-unique) included) be carried
by $c_i$ sequences, so its frequency is $\hat p_i = c_i/N$, and let $s_{ij}$ be the similarity of
variants $i$ and $j$ (below). Then [@wu2025, Section 2.1 and Supplementary File 1]

$$
\begin{equation}
S_i = \sum_{j \ne i} \hat p_j\, s_{ij}, \qquad
H_\text{sim} = \sum_i \left(-\hat p_i \log_2 \hat p_i\right)\left(1 - S_i\right) .
\end{equation}
$$

$S_i$ is the average, over all $N$ sequences at the position, of their similarity to variant
$i$, with the variant's own sequences counted as 0; $1 - S_i$ is the variant's weight. Wu uses
other letters for two of these quantities [@wu2025, Sections 1 and 2.1]:

| Quantity | This page | Wu 2025 |
|---|---|---|
| Sequences at the position (support) | $N$ | $T$ (clones) |
| Distinct k-mers at the position | $D$ | $N$ (unique sequences) |
| Support threshold | $T$ | — |

### The similarity of two k-mers

Each of the $k$ sites of a k-mer holds one residue. The similarity of two k-mers is the proportion
of their $k$ sites that carry the same residue, $s_{ij} = (k - d_{ij})/k$, where $d_{ij}$ is the
number of sites at which they differ [@wu2025, Section 2.1, Fig. 5b and Supplementary File 1]
(Section 2.1's wording describes a distance; the reading $1 - d/k$ comes from Wu's printed
values). For amino acids this is the fraction of identical residues of two aligned protein
sequences [@grishin1995, p. 675]. Because k-mers have equal length and no gaps, the usual
definitions of percentage identity, which differ only in how they count gaps and unequal lengths,
all give this value [@raghava2006, Methods, Eqs. 2–5]. Identity treats every substitution alike: a
conservative exchange lowers the similarity as much as a radical one, and percentage identity is
"a poor substitute for more sophisticated scoring methods that take account of the
physico-chemical properties of the amino acids" [@raghava2006, Conclusion]. It is not an
evolutionary distance either: it counts the sites that differ now, not the substitutions that
occurred, some of which may have been reversed [@grishin1995, pp. 675–676].

Wu computes each variant's weight from the similarity of every pair of distinct sequences
[@wu2025, Sections 2.1 and 2.3]; DiMA obtains exactly the same sums from the residue counts at
each site of the k-mer, without forming pairs (see
[Numerical accuracy](/explanation/numerical-accuracy/#similarity-weighted-entropy)).

### Weights and bounds

Written with the distances, a variant's weight is its own frequency plus the average share of
sites at which a sequence of the position differs from it:

$$
\begin{equation}
1 - S_i = \hat p_i + \sum_j \hat p_j\, \frac{d_{ij}}{k} .
\end{equation}
$$

Two distinct k-mers differ at one site at least and at most at all $k$, so every weight lies
between $\hat p_i + (1 - \hat p_i)/k$ and 1. $H_\text{sim}$ is therefore never more than the
observed entropy $\hat H$, and equals it when every pair of variants differs at every site (always
for $k = 1$). Two equally common variants that differ at $d$ sites give
$H_\text{sim} = (k + d)/(2k)$ bits, against $\hat H = 1$ bit.

### Normalised values

DiMA also reports both entropies divided by $\log_2 D$, the largest observed entropy possible
with the $D$ distinct k-mers at the position (the Index included): $H_n = \hat H / \log_2 D$ and
$H_\text{nsim} = H_\text{sim} / \log_2 D$, each between 0 and 1 [@wu2025, Section 2.1 and
Supplementary File 1].
$H_\text{nsim}$ reaches 1 only when every pair of k-mers differs at every site, which needs no
more distinct k-mers than letters. Wu notes
that entropy is commonly normalised by $\log_2$ of the number of clones, or by $\log_2$ of "the
estimated number of unique sequences", the latter "particularly common in next generation
sequencing (NGS) data analyses" [@wu2025, Section 1]; his $H_n$ divides by $\log_2$ of "the total
number of unique sequences in the population" [@wu2025, Section 2.1], which DiMA takes as the
observed number of distinct k-mers $D$. $H_n$ has the form of Pielou's evenness
$J' = H'/\log s$, with $D$ in place of the number of species $s$ of the population
[@pielou1966, pp. 141 and 143]; Sheldon already divided by the logarithm of the observed number of
species [@sheldon1969, p. 467].

With a single distinct k-mer ($D = 1$) the largest possible entropy is 0, so the normalised values
are undefined (`null`) while $H_\text{sim} = 0$, as Pielou left the evenness of a quadrat (a
sampling plot of a plant community) with a
single species undefined [@pielou1966, p. 141]. Without support all three values are `null`.

The normalised values depend on $D$ as well as on how evenly the sequences are spread, especially
when $D$ is small: equitability indices (evenness measures such as $H_n$, with $S$ the number of
species, here $D$) are "fairly stable for S > 20" [@sheldon1969, p. 467],
while $J'$, the form of $H_n$, depends on the number of species "up to S = 25 or higher"
[@smith1996, Requirement 1, p. 72].

### Worked example

At MERS-CoV spike position 25 ($k = 9$, support 170, 10 distinct 9-mers; Table 1 below lists
their counts) the Index `SVKSACIEV`, carried by 129 sequences, has weight 0.786: its own
frequency, 129/170 = 0.759, plus 0.027 for the 41 other sequences, 40 of which differ from it at
one of the nine sites and one at two ($42/(170 \times 9)$). The eight
variants one substitution from the Index have weights between 0.127 and 0.195, and `SAKSVCIEV`,
two substitutions from it, 0.231. So $\hat H = 1.4224$ bits, $H_\text{sim} = 0.4275$ bits,
$H_n = 1.4224/\log_2 10 = 0.4282$ and $H_\text{nsim} = 0.1287$. Over the whole protein (all 1,345
positions eligible) the mean similarity-weighted entropy is 0.0363 bits, against a mean observed
entropy of 0.1207 bits over the same positions; it is 0 at the 455 positions with a single 9-mer
and highest, 0.4548 bits, at position 26. `dima analyze --similarity-weighted-entropy` ends its
summary line with `similarity-weighted mean 0.0363 bits (observed 0.1207)`.

### k and the alphabet

The values depend on $k$ and on the alphabet. With $k = 1$ the similarity-weighted and observed
entropies are equal; with longer k-mers, distinct k-mers usually differ at a smaller share of
their sites, so the weights, and $H_\text{sim}$ relative to $\hat H$, are lower (on the MERS-CoV
spike protein at $k = 9$, the mean similarity-weighted entropy is 30% of the mean observed
entropy). A nucleotide and an amino-acid k-mer of the same $k$ cover different lengths of genome,
and a synonymous change alters the nucleotide k-mer only. Compare values only between runs with
the same $k$ and alphabet.

### Relation to other measures

$H_\text{sim}$ takes similarity into account, as the family of similarity-sensitive diversity
measures of Leinster and Cobbold does [@leinster2012, p. 479], but it is not a member of that
family: with their similarity matrix $Z_{ij} = s_{ij}$ ($Z_{ii} = 1$),
$S_i = (Z\hat p)_i - \hat p_i$, and $H_\text{sim}$ is an entropy in bits, not an effective number
of variants (the number of equally common, entirely dissimilar variants that would have the same
diversity). It is also not to be confused with the "similarity-weighted entropy analysis" of
Mo et al. [@mo2026, Methods, "Entropy of VP1 codons and physical property analyses of high-entropy
codons", and Results], which gives near-identical sequences less weight and then computes Shannon
entropy column by column.

### What the evidence covers

Wu defines a viral quasispecies as "a population of related viral sequences within an infected
host" [@wu2025, Section 1], and applied the metric only to nucleotide sequences of equal length
that differ by substitutions only: simulated populations, a potato spindle tuber viroid (PSTVd)
mutant pool and the populations it gave within single plants, and two tomato brown rugose fruit
virus (ToBRFV) samples [@wu2025, Sections 2.1 and 3.1–3.3]. DiMA's alignments usually hold sequences
from many hosts, and DiMA applies the metric at every k-mer position, to amino-acid k-mers too.
Wu's reading, that a variant similar to the others adds less to a quasispecies' diversity, then
becomes a statement about the k-mers sampled across hosts. These uses are DiMA's extension and are
untested.

As of 25 November 2026, searches of OpenAlex, Semantic Scholar, OpenCitations and Europe PMC
(including full text; Crossref's citation count agrees) found no published application or
evaluation of the metric by other authors; the works that cite it are reviews. The data of both
of Wu's real examples are published as unique sequences with their read counts. With them DiMA
reproduces his PSTVd values (Fig. 5c, as far as the figure can be read), and his ToBRFV values for
one sample but not for the other (see [Comparison with the papers](/explanation/paper-comparison/#similarity-weighted-entropy-wu-2025)).

### How the values change with the number of sequences

The three values describe the k-mers observed at a position, not the population they were
sampled from, and none of them is corrected for sample size. Observed entropy is lower on average
with fewer sequences [@zhao2019, Section 3.1], normalised entropy also depends on sample size
[@gregori2014, Sections 2.4 and 3.2], and similarity-weighted entropy changes too: with fewer
sequences the values can be higher or lower. Table 1 shows this at MERS-CoV spike position 25 by
averaging each value over every subset of $n$ of the position's sequences; its last row is the
position itself. The observed and similarity-weighted entropies fall as $n$ falls, while the
normalised values rise.

<!-- docs-numbers:swe-subsets:start -->

**Table 1.** MERS-CoV spike position 25 (170 sequences; its 10 distinct 9-mers are carried by 129,
14, 9, 6, 4, 4, 1, 1, 1 and 1 of them): the average over all subsets of $n$ of its sequences,
computed exactly. The normalised values are averaged over the subsets with at least two distinct
9-mers.

| $n$ | $\hat H$ | $H_\text{sim}$ | $H_n$ | $H_\text{nsim}$ | Distinct 9-mers | Subsets with one 9-mer |
|---|---|---|---|---|---|---|
| 10 | 0.9979 | 0.3812 | 0.6494 | 0.2537 | 2.99 | 5.80% |
| 25 | 1.2126 | 0.4053 | 0.5372 | 0.1812 | 4.83 | 0.05% |
| 50 | 1.3201 | 0.4164 | 0.4907 | 0.1553 | 6.52 | < 0.01% |
| 100 | 1.3878 | 0.4236 | 0.4565 | 0.1395 | 8.29 | < 0.01% |
| 170 | 1.4224 | 0.4275 | 0.4282 | 0.1287 | 10.00 | 0% |

<!-- docs-numbers:swe-subsets:end -->

Table 2 compares the averages with the values of two known populations instead, for samples
drawn at random with replacement. With the population of Wu's Fig. 1b the normalised values rise
as $n$ falls; with two equally common 9-mers they fall.

<!-- docs-numbers:swe-populations:start -->

**Table 2.** The values of two populations, and their average over samples of $n$ sequences drawn at
random with replacement, computed exactly. (a) Quasispecies 2 of Wu's Fig. 1b [@wu2025, Fig. 1b]: a
master sequence at 90% and four variants at 4%, 4%, 1% and 1% that differ from it at 2, 4, 2 and 4
sites, no two variants at the same site, as drawn (so $k = 12$, the shortest length that holds
them). (b) Two 9-mers at 50% each that differ at one site. The normalised values are averaged over
the samples with at least two distinct k-mers.

| Population | $n$ | $\hat H$ | $H_\text{sim}$ | $H_n$ | $H_\text{nsim}$ | Samples with one k-mer |
|---|---|---|---|---|---|---|
| (a) | 10 | 0.4328 | 0.2195 | 0.5474 | 0.2789 | 34.87% |
| (a) | 25 | 0.5293 | 0.2453 | 0.3777 | 0.1756 | 7.18% |
| (a) | 50 | 0.5791 | 0.2579 | 0.3214 | 0.1434 | 0.52% |
| (a) | 100 | 0.6093 | 0.2654 | 0.2954 | 0.1288 | < 0.01% |
| (a) | population | 0.6412 | 0.2733 | 0.2761 | 0.1177 | — |
| (b) | 10 | 0.9235 | 0.4970 | 0.9253 | 0.4980 | 0.20% |
| (b) | 25 | 0.9705 | 0.5318 | 0.9705 | 0.5318 | < 0.01% |
| (b) | 50 | 0.9854 | 0.5437 | 0.9854 | 0.5437 | < 0.01% |
| (b) | 100 | 0.9927 | 0.5496 | 0.9927 | 0.5496 | < 0.01% |
| (b) | population | 1.0000 | 0.5556 | 1.0000 | 0.5556 | — |

<!-- docs-numbers:swe-populations:end -->

**Why DiMA reports no corrected values.** Wu defines plug-in values, and the published method is
DiMA's specification ([working agreement 1](/contributing/working-agreements/)). No
published or validated correction exists for $H_\text{sim}$, and all three values depend on
support: observed and normalised entropy [@zhao2019, Section 3.1; @gregori2014, Sections 2.4 and
3.2], and similarity-weighted entropy too (Tables 1 and 2). Nor can any of them be
estimated without bias: from $n$ sequences drawn independently at random, only polynomials of the
variant frequencies can be estimated without bias, and Shannon entropy is not one
[@paninski2003, Appendix A.2, Proposition 8]. $H_\text{sim}$ is not one either: for two k-mers of
similarity $s < 1$ at frequencies $p$ and $1 - p$,
$H_\text{sim} = -p \log_2 p\,(1 - s + sp) - (1 - p)\log_2(1 - p)\,(1 - sp)$, whose slope grows
without bound as $p \to 0$, which a polynomial's never does.

**Comparing positions.** Because the values depend on support, positions of very different
support can rank differently from their populations, as shown for observed and normalised entropy
[@zhao2019, Section 3.1; @gregori2014, Section 3.2]. Compare positions of similar support and
similar sample coverage (both are reported): equally complete samples can be compared directly,
as shown for species richness [@chao2012, p. 2538]. For quasispecies data, Gregori et al. recommend rarefying the larger sample
to the size of the smaller by repeated subsampling without replacement [@gregori2014, Sections
2.5 and 3.2]; DiMA does not.

The summary (`similarity_weighted_entropy_summary`: the mean, lowest and highest
similarity-weighted entropy, and the mean observed entropy of the same positions) covers the
eligible positions only (support above $T$), as the entropy summaries do; normalised values are
not summarised. At positions with support at or below $T$ the desktop app marks the two
normalised values *(low support)*: they come from few sequences, are less reliable and can be
higher or lower than with more sequences. Where every sequence carries a different k-mer,
$H_n = 1$ by arithmetic alone ($\hat H = \log_2 D$), whatever the sequences.

### Where the paper and DiMA differ

DiMA follows the reading of Wu's formulas that reproduces his Supplementary File 1. Some details
of the paper differ from it, or from Wu's data (the numbers are in
[Comparison with the papers](/explanation/paper-comparison/#similarity-weighted-entropy-wu-2025)):

- Section 2.1 describes $S_{ij}$ as "the number of base substitutions between sequences i and j,
  normalized by the sequence length", which is a distance; Wu's printed values use the similarity
  $1 - d/k$.
- Section 2.1 prints the average similarity as $\sum_{i \ne i} f_i s_{ij}$. Summed over the other
  variants $j$ with the variant's own frequency $f_i$, as printed, it gives a negative
  $H_\text{sim}$ for Fig. 2's Distributions 1 and 2 at similarity 0.999 (−0.39 and −0.88 bits);
  weighted by $f_j$, as above, it reproduces Supplementary File 1.
- Section 2.1 prints the formula of $H_\text{nsim}$ as that of $H$ and says it "normalizes
  H_sim by log2(T)"; Supplementary File 1 divides by $\log_2$ of the number of distinct
  sequences, as $H_n$ does.
- Fig. 4's caption puts $H$ and $H_n$ in panel (a) and $H_\text{sim}$ and $H_\text{nsim}$ in
  panel (b); the panels and the text (Section 3.1) show $H$ and $H_\text{sim}$ in (a), $H_n$ and
  $H_\text{nsim}$ in (b).
- Fig. 5b's similarities (0.75, 0.50, 0.25 and 0.00) are those of 4-site sequences, while the
  printed $H_\text{sim} = 4.04$ and $H_\text{nsim} = 0.50$ of the theoretical UNNNNA pool need all
  six sites; exactly, they are 4.03125 and 0.50390625.
- Two cells of Supplementary File 1 hold the values of another similarity: in the "Figure 2
  Extended Data" sheet (whose five populations are further ones, not Fig. 2's), distribution 1's
  values at similarity 0.3 are those at 0.4, and in the "Figure 3 Extended Data" sheet, D-1's
  values at 0.7 are those at 0.95 (Fig. 3b).
- From Wu's published ToBRFV data (the unique sequences with their read counts), $H$ and $H_n$
  are higher in the Tm-2² sample than in the tm-2 sample, the opposite of Section 3.3.
- Fig. 6's legend says that sequences below 0.1% of the reads were excluded, but the published
  tm-2 data list eight sequences with 10 of 10,833 reads (0.092%); with all 48, the tm-2 values
  of Fig. 6d are reproduced.

## Numerical details

Logarithms come from the `libm` library, so they give the same bits on every platform; sums use
compensated (Neumaier) summation; each position is computed by one thread and summaries are
reduced in position order. Identical inputs therefore give byte-identical outputs.

Rounding error is present in every value, as in any floating-point program, and grows with the
number of sequences at a position. On the bundled datasets every entropy is within about
$5 \times 10^{-15}$ of the exact value (relative), and at the 2.2 million sequences per position
of the paper's SARS-CoV-2 analysis within about $8 \times 10^{-12}$. That is $10^{10}$ to
$10^{12}$ times smaller than the sampling error of the estimate. Similarity-weighted entropy has
the same absolute error as observed entropy, but is smaller, so its relative error is larger: up
to $2 \times 10^{-14}$ on the bundled datasets and $1.2 \times 10^{-10}$ at 2.2 million sequences.
[Numerical accuracy](/explanation/numerical-accuracy/) gives the measurements, the one limit
(more than $9 \times 10^{15}$ sequences at a position) and why a prototyped fix for the last
digits is deferred.

## References

[^ref]
