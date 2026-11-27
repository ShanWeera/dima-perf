---
title: Stratify by header metadata
description: Count every variant per host, country, year or any other field of the FASTA headers — header formats, cleaning, fill values, mismatched headers and identifier-like fields.
---

DiMA can count every variant per value of fields stored in the FASTA headers, for example to see
which hosts or years carry a variant [@tharanga2025]. This guide shows how to describe your
headers, what DiMA does with their values, and how to avoid the common pitfalls.

## Describe the headers

Headers must hold their fields separated by `|`. Name the fields, in order, with
`--header-format`:

```text
>YP_009047204.1 |Homo sapiens||2012
>AGN52936.1 |Homo sapiens|United Arab Emirates|2013
>QCZ25046.1 |Camelus bactrianus|Saudi Arabia|2016
```

```sh
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/samples/mers_spike_aa.fasta
dima analyze -i mers_spike_aa.fasta \
    --header-format 'Accession|Host|Geolocation|Year' \
    --metadata-fields 'Host|Geolocation|Year' \
    -o mers.json --tsv-dir tables
```

- `--header-format` names every field of the header, in order. Names must be non-empty and
  distinct. The paper's examples are Accession, Strain, Species, Year, Geolocation and Host, but
  any names work. Names are trimmed and [NFC](/reference/glossary/#nfc)-normalised like the
  values, so `--header-format 'Accession| Host |Geolocation|Year'` works with
  `--metadata-fields Host`.
- `--metadata-fields` chooses the [stratified fields](/reference/glossary/#stratified-field): the
  fields to count variants by (default: all of them). Use it to leave out identifiers such as
  accessions (see below). Its order does not matter; results follow the header-format order. In
  the desktop app, tick the **Stratify** boxes of the header editor.
- The whole header line after `>` is split, including what other tools call the description:
  `YP_009047204.1 ` (with its trailing space) is the first field above.

Every variant then carries, for each stratified field, the number of its sequences per value. At
position 25 of the MERS-CoV spike sample, the 14 sequences of the Major variant `SFKSACIEV`
split as:

```json
"metadata": {
  "Host": { "Camelus": 2, "Camelus dromedarius": 9, "Homo sapiens": 3 },
  "Geolocation": { "Oman": 2, "United Arab Emirates": 12 },
  "Year": { "2014": 3, "2015": 11 }
}
```

The paper's [relative incidence](/reference/glossary/#relative-incidence) of a value is its
count divided by the variant's count: here 9/14 = 64.3% of the Major variant's sequences come from
*Camelus dromedarius* [@tharanga2025]; the desktop app's metadata panel shows it as "64.3% (9)".
In tables, `metadata.tsv` has one row per position, variant, field and value; see
[TSV tables](/reference/tsv/#metadata).

Values are compared literally, after cleaning: `Camelus` and `Camelus dromedarius` are two hosts,
and so is the placeholder `a` that two MERS-CoV records carry. Make such spellings consistent in
the headers before the analysis if they mean the same.

### The background mix at a position

A variant's counts can be set beside the mix of all the sequences at its position — the
background of hosts, countries or years from which its sequences come. The counts of all the
variants of a position add up to that mix. With the tables written above, this prints the hosts
of every sequence with a valid k-mer at position 25, with their share:

```sh
awk -F'\t' '$1 == 25 && $3 == "Host" { n[$4] += $5; total += $5 }
     END { for (v in n) printf "%d\t%.1f%%\t%s\n", n[v], 100 * n[v] / total, v }' \
    tables/mers_spike_aa.metadata.tsv | LC_ALL=C sort -k1,1nr
```

```text
93	54.7%	Homo sapiens
57	33.5%	Camelus dromedarius
16	9.4%	Camelus
2	1.2%	Camelus bactrianus
2	1.2%	a
```

The 170 sequences are the position's support. *Camelus dromedarius* carries 9 of the 14
`SFKSACIEV` sequences (64.3%) but 57 of the 170 at the position (33.5%). The same recipe works
from saved results: `dima view -i mers.json -O tsv --table metadata` writes the table to standard
output.

### Read the counts with care

The paper calls a value's share of a variant's own sequences its **relative incidence**
[@tharanga2025, Methods]: the 64.3% above. Comparing it with the position's mix (33.5%) shows that
the Major variant is found more often in dromedaries than the position as a whole, but DiMA does
not test that difference, and neither the paper nor the studies it builds on define a measure of
enrichment for it. The paper goes no further than *"Such a distribution may suggest regional
adaptation and selective pressures favoring the specific 9-mer"* [@tharanga2025, Results]. Treat a
difference as something to look into, not as a result, for three reasons:

- **The counts describe the sequences in your file, which describe who sequenced what.** For
  SARS-CoV-2, high-income countries submitted ten times more sequences per reported case than
  low- and middle-income countries, and the number of lineages seen in a country followed the
  number of its genomes [@brito2022, Results]. Even the numbers of genomes needed to detect a
  lineage, worked out for representative sampling, *"can only tell if a lineage is present, not
  how prevalent it is"* [@brito2022, Results]. A variant counted in a group shows that it was
  sampled there; how common it is there depends on how the group was sampled.
- **Public databases hold duplicates** of the same isolate, which the paper suggests checking by
  analysing the data with and without them [@tharanga2025, Methods], and they often record little
  about each isolate's history [@hu2013, Methods].
- **Small numbers.** A share of 64.3% here rests on 14 sequences; *"Ideally, large sample size is
  preferred to mitigate the effect of any potential data biases"* [@tharanga2025, Methods].

In practice: report a variant's counts *and* the position's totals for the same field, as above;
expect a field that one value dominates (99% of the sequences from one host) to show that value
for almost every variant; compare shares over time as the paper does, the Index's falling
monthly share against the Major variant's rising one over the same months [@tharanga2025,
Results]; and to compare hosts or countries, split the alignment and analyse each group on its
own, as the DiMA lineage compared avian and human influenza [@raman2020, Discussion] (see
[How do I get the entropy per host, country or year?](/getting-started/faq/#how-do-i-get-the-entropy-per-host-country-or-year)).

### Dates and other fine-grained values

Each distinct value is a separate stratum, so a field of full dates (`2014-05-12`) counts variants
per day. To count them per year, shorten the field in the headers first; with the date as the
fourth field:

```sh
awk -F'|' 'BEGIN { OFS = "|" } /^>/ { $4 = substr($4, 1, 4) } { print }' dated.fasta > years.fasta
```

A header `>MN1.1|Homo sapiens|Saudi Arabia|2014-05-12` becomes
`>MN1.1|Homo sapiens|Saudi Arabia|2014`; sequence lines are unchanged.

### Headers with other separators

Convert other layouts to `|` first. Alignments from the Los Alamos HIV Sequence Database
[@kuiken2003], for example, name sequences `Subtype.Country.Year.Name.Accession`, where the name
itself may contain dots:

```sh
awk '/^>/ { n = split(substr($0, 2), f, ".");
            name = f[4]; for (i = 5; i < n; i++) name = name "." f[i];
            print ">" f[1] "|" f[2] "|" f[3] "|" name "|" f[n]; next }
     { print }' hiv.fasta > hiv.pipes.fasta

dima analyze -i hiv.pipes.fasta \
    --header-format 'Subtype|Country|Year|Name|Accession' \
    --metadata-fields 'Subtype|Country|Year' -o hiv.json.zst
```

## How values are cleaned

Each value is cleaned before counting: invisible characters (zero-width spaces, bidirectional
marks, control characters other than tab) are removed, white space is trimmed from both ends, and
the text is normalised to Unicode NFC, so that visually identical spellings count as one value.
Nothing else is changed. An **empty** value becomes the [fill value](/reference/glossary/#fill-value),
`Unknown` by default; the fill value is trimmed and NFC-normalised too:

```sh
dima analyze -i data.fasta --header-format 'Accession|Host|Country|Year' \
    --header-fillna 'not recorded' -o data.json
```

In the MERS-CoV sample, the first record has an empty Geolocation: it is counted as `Unknown`.
What was changed is reported (`header_values_cleaned`, with `-v`) and counted in
`run.validation.header_cleaning`. See [header metadata](/explanation/input/#header-metadata).

## Headers with the wrong number of fields

A header whose number of `|`-separated fields differs from `--header-format` is a problem with the
data, and the default (strict) validation stops the run after reading all headers, listing the
first 20 offenders with their record and line numbers. You can then fix the headers, or the
format. When no header matches, the format is probably wrong, and the hint says so first. Giving
the four-field MERS-CoV headers only two names:

```sh
dima analyze -i mers_spike_aa.fasta --header-format 'Accession|Host' -o mers.json
```

```text
error: mers_spike_aa.fasta: 191 headers do not have the 2 fields of the header format (Accession|Host)
  record 1 (line 1): 4 fields: YP_009047204.1 |Homo sapiens||2012
  record 2 (line 25): 4 fields: AGN52936.1 |Homo sapiens|United Arab Emirates|2013
  …
  record 20 (line 457): 4 fields: ALJ54520.1 |Homo sapiens|Saudi Arabia|2015
  and 171 more
  hint: no header has the 2 fields of --header-format, so the format is probably wrong: check it against the headers; otherwise fix the headers, or use --validation permissive to give mismatched headers the fill value
```

The run stops with exit code 4 and writes no results.

With `--validation permissive`, such headers are accepted: all their stratified fields get the
fill value, and one warning (`header_field_count_mismatch`) reports how many there were;
`run.metadata.mismatches` lists the first 20.

```sh
dima analyze -i data.fasta --header-format 'Accession|Host|Country|Year' \
    --validation permissive -o data.json
```

:::caution[Permissive mode accepts two problems at once]
`--validation permissive` is one switch for two different problems: characters outside the
alphabet (their k-mers are excluded) and headers with the wrong number of fields (their
stratified values become the fill value). Each is reported by its own warning
(`unknown_characters_excluded`, `header_field_count_mismatch`), but neither stops the run any
more, so read the warnings before you use the results. See
[Validate input characters](/how-to/validation/).
:::

## Warnings about values

DiMA checks the values of every stratified field and warns about three common problems. The
analysis is not changed by them; they are there for you to decide.

### Identifier-like fields

A field whose value differs in every record — an accession, a strain name — adds one count per
sequence to every variant. Results grow with the number of sequences, the metadata table can
become very large, and the counts are rarely what was meant. When a stratified field has a
different value in each of the first records (at least 100 are needed), DiMA warns
(`identifier_like_field`) and prints the exact `--metadata-fields` option without it:

```text
warning[identifier_like_field]: field "Accession" had a different value in each of the first 191
records; identifier-like fields add one metadata entry per sequence to every variant, which makes
the results large
  e.g. Accession
  hint: to stratify without them: --metadata-fields 'Host|Geolocation|Year'
```

All declared fields are stratified unless you choose otherwise, because the paper's examples
include the accession and some analyses need it. When every stratified field is identifier-like,
the hint reads "every stratified field is identifier-like; omit --header-format if no
stratification is needed".

### Values that mean "missing"

Values such as `NA`, `N/A`, `NaN`, `null`, `None`, `missing` or the INSDC terms `not collected`,
`not provided`, `not applicable`, `restricted access` (in any case) usually mean "missing", but
DiMA cannot know that, so it keeps them as values and warns (`missing_like_values`). In the
MERS-CoV sample, two records have the year `nan`. If they mean missing, empty those fields in the
headers so they receive the fill value, or edit them consistently (the hint's words). A value equal
to the fill value is never reported, so with `--header-fillna NA` the value `NA` raises no
warning.

### Near-duplicate spellings

Values that differ only in case, spacing or character forms — `Saudi Arabia` and
`saudi  arabia` — are counted separately; DiMA lists such groups (`near_duplicate_values`). Make
the spellings consistent in the headers if they mean the same.

## Size of the output

Metadata multiplies the output: every variant carries one count per value. For example, the
HIV-1 *pol* DNA alignment of the paper (2,637 sequences) stratified by subtype, country and year
gives a `metadata.tsv` of about 230 MB. Compress the JSON (`-o results.json.zst`), stratify only
by the fields you need, and see [Analyse large alignments](/how-to/large-alignments/).

## References

[^ref]
