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
dima analyze -i samples/mers_spike_aa.fasta \
    --header-format 'Accession|Host|Geolocation|Year' \
    --metadata-fields 'Host|Geolocation|Year' \
    -o mers.json --tsv-dir tables
```

- `--header-format` names every field of the header, in order. Names must be non-empty and
  distinct. The paper's examples are Accession, Strain, Species, Year, Geolocation and Host, but
  any names work.
- `--metadata-fields` chooses the fields to count variants by (default: all of them). Use it to
  leave out identifiers such as accessions (see below). Its order does not matter; results follow
  the header-format order.
- The whole header line after `>` is split, including what other tools call the description:
  `YP_009047204.1 ` (with its trailing space) is the first field above.

Every variant then carries, for each counted field, the number of its sequences per value. At
position 25 of the MERS-CoV spike sample, the 14 sequences of the Major variant `SFKSACIEV`
split as:

```json
"metadata": {
  "Host": { "Camelus": 2, "Camelus dromedarius": 9, "Homo sapiens": 3 },
  "Geolocation": { "Oman": 2, "United Arab Emirates": 12 },
  "Year": { "2014": 3, "2015": 11 }
}
```

The paper's relative incidence of a value is its count divided by the variant's count: here
9/14 = 64.3% of the Major variant's sequences come from *Camelus dromedarius* [@tharanga2025].
In tables, `metadata.tsv` has one row per position, variant, field and value; see
[TSV tables](/reference/tsv/#metadata).

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
Nothing else is changed. An **empty** value becomes the **fill value**, `Unknown` by default:

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
format.

With `--validation permissive`, such headers are accepted: all their counted fields get the fill
value, and one warning (`header_field_count_mismatch`) reports how many there were;
`run.metadata.mismatches` lists the first 20.

```sh
dima analyze -i data.fasta --header-format 'Accession|Host|Country|Year' \
    --validation permissive -o data.json
```

## Warnings about values

DiMA checks the values of every counted field and warns about three common problems. The
analysis is not changed by them; they are there for you to decide.

### Identifier-like fields

A field whose value differs in every record — an accession, a strain name — adds one count per
sequence to every variant. Results grow with the number of sequences, the metadata table can
become very large, and the counts are rarely what was meant. When a counted field has a different
value in each of the first records (at least 100 are needed), DiMA warns
(`identifier_like_field`) and prints the exact `--metadata-fields` option without it:

```text
warning[identifier_like_field]: field "Accession" had a different value in each of the first 191
records; identifier-like fields add one metadata entry per sequence to every variant, which makes
the results large
  e.g. Accession
  hint: to stratify without them: --metadata-fields 'Host|Geolocation|Year'
```

All declared fields are counted unless you choose otherwise, because the paper's examples include
the accession and some analyses need it.

### Values that mean "missing"

Values such as `NA`, `N/A`, `NaN`, `null`, `None`, `missing` or the INSDC terms `not collected`,
`not provided`, `not applicable`, `restricted access` (in any case) usually mean "missing", but
DiMA cannot know that, so it keeps them as values and warns (`missing_like_values`). In the
MERS-CoV sample, two records have the year `nan`. If they mean missing, empty those fields in the
headers so they receive the fill value, or make them consistent.

### Near-duplicate spellings

Values that differ only in case, spacing or character forms — `Saudi Arabia` and
`saudi  arabia` — are counted separately; DiMA lists such groups (`near_duplicate_values`). Make
the spellings consistent in the headers if they mean the same.

## Size of the output

Metadata multiplies the output: every variant carries one count per value. For example, the
HIV-1 *pol* DNA alignment of the paper (2,637 sequences) stratified by subtype, country and year
gives a `metadata.tsv` of about 230 MB. Compress the JSON (`-o results.json.zst`), count only the
fields you need, and see [Analyse large alignments](/how-to/large-alignments/).

## References

[^ref]
