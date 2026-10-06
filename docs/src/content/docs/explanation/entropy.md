---
title: Entropy and its correction
description: Shannon entropy of k-mer variants, the paper's sample-size correction (Fig. 2) and why DiMA computes its exact expectation, the alternative estimators, sample coverage, and why positions without support have no entropy.
---

At every position DiMA summarises the distribution of variants by its Shannon entropy
[@shannon1948]: 0 bits when every sequence carries the same k-mer, more as variants become more
numerous and more even. This page derives the observed entropy, the paper's correction for small
samples [@tharanga2025], the way DiMA evaluates that correction exactly, the optional
alternative estimators, and the sample coverage reported next to them.

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
  about 5.35, and repeated runs of the random procedure give 5.350–5.353 (see
  [Comparison with the paper](/explanation/paper-comparison/)).
- **Fig. 1C.** At MERS-CoV spike position 25 the paper shows 1.38 bits ($T = 30$). The exact
  expectation is 1.4451 bits. A simulation of the paper's random procedure at that position
  (20,000 runs) has a standard deviation of 0.22 bits, and 1.38 lies at about its 39th
  percentile: an ordinary draw of the random procedure, not a different method. With $T = 100$
  the expectation is 1.4617 bits (standard deviation of the random procedure 0.30 bits).

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

DiMA v5.0.9 applies the rule to each random run instead. Where one or a few sequences carry a
variant other than the Index, a single run's intercept is often negative and is then replaced by
the larger observed entropy, so v5.0.9's average over many runs is higher there. With the full
eleven-point grid, about 20% of runs fall back when one sequence differs, raising the average by
34–41%; 12–15% with two sequences (17–23%); and 3–5% with five (3–6%), on values of a few
hundredths of a bit. With fewer points ($N$ close to $T$) the effect is larger: one sequence
differing raises the average by 51% at $N = 171$ and by 107% at $N = 130$ ($T = 100$).
On the MERS-CoV spike protein ($T = 100$) the eligible mean is 0.1261 bits here and about 0.142
bits averaged over v5.0.9 runs; on the HIV-1 *pol* datasets the two agree. In simulations of
two-variant populations with a known entropy ($N$ from 171 to 3,000, minor-variant frequency
0.05% to 5%), the per-run rule overestimated by up to 19% on average, while the expectation
underestimated, by up to 27% when less than one minor-variant sequence was expected and by at
most 3% otherwise, with an error close to that of the Miller–Madow correction; neither had the
smaller error everywhere.

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

## Numerical details

Logarithms come from the `libm` library, so they give the same bits on every platform; sums use
compensated (Neumaier) summation; each position is computed by one thread and summaries are
reduced in position order. Identical inputs therefore give byte-identical outputs.

Rounding error is present in every value, as in any floating-point program, and grows with the
number of sequences at a position. On the bundled datasets it is within about
$5 \times 10^{-15}$ of the exact values (relative), and at the 2.2 million sequences per position
of the paper's SARS-CoV-2 analysis within $2.2 \times 10^{-12}$. That is $10^{10}$ to
$10^{12}$ times smaller than the sampling error of the estimate. [Numerical accuracy](/explanation/numerical-accuracy/)
gives the measurements, the one limit (more than $9 \times 10^{15}$ sequences at a position) and
why a prototyped fix for the last digits is deferred.

## References

[^ref]
