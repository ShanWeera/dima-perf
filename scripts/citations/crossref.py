#!/usr/bin/env python3
"""Check the stored bibliographic records against Crossref (stdlib only).

Every DOI-bearing reference in ``crates/dima-core/src/citations_data.rs`` has a verified record
in ``xtask/data/crossref_snapshot.json``; ``cargo xtask gen --mode check`` keeps the Rust data and
the snapshot identical offline. This script closes the loop online: it re-fetches each DOI from
the Crossref REST API, normalises the record, applies the hand-verified CORRECTIONS below
(Crossref metadata is occasionally incomplete or mis-encoded) and reports any field that differs
from the snapshot. Run weekly in CI (non-gating) and before adding references.

Usage: python3 scripts/citations/crossref.py [--snapshot PATH] [--doi DOI ...]
Exit status: 0 no drift, 1 drift or unresolvable DOI, 2 usage/network setup error.
"""
import argparse
import html
import json
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SNAPSHOT = ROOT / "xtask" / "data" / "crossref_snapshot.json"
# No personal data in requests: a fixed, anonymous User-Agent.
USER_AGENT = "dima-citation-check/1.0"

# Hand-verified corrections against the publishers' pages (field -> corrected value).
CORRECTIONS = {
    "10.1038/s41592-018-0046-7": {"authors": [["Grüning", "Björn"], ["Dale", "Ryan"], ["Sjödin", "Andreas"], ["Chapman", "Brad A."], ["Rowe", "Jillian"], ["Tomkins-Tinch", "Christopher H."], ["Valieris", "Renan"], ["Köster", "Johannes"], ["Bioconda Team", ""]]},
    "10.1093/gigascience/giy125": {"volume": "7", "issue": "12", "pages": "giy125"},
    "10.1214/ss/1009213286": {"pages": "101-133"},
    "10.1214/aos/1176344552": {"pages": "1-26"},
    "10.18637/jss.v059.i10": {"pages": "1-23"},
    "10.2307/2333344": {"pages": "237-264"},
    "10.1093/bib/bbae607": {"year": 2025},  # issue year; published online 26 Nov 2024
    # Crossref splits "Van Vleck" and gives first pages or a page too many (checked on the articles).
    "10.2307/1967627": {"authors": [["Van Vleck", "Edward B."]], "pages": "1-18"},
    "10.32614/RJ-2015-022": {"authors": [["Hankin", "Robin K. S."]], "pages": "81-88"},
    "10.1090/S0025-5718-97-00807-7": {"pages": "373-389"},
    "10.1093/biomet/26.4.404": {"authors": [["Clopper", "C. J."], ["Pearson", "E. S."]], "title": "The use of confidence or fiducial limits illustrated in the case of the binomial"},
    "10.1093/biomet/63.3.581": {"authors": [["Rubin", "Donald B."]]},
    "10.1021/acs.jproteome.5b00981": {"title": "Accounting for the multiple natures of missing values in label-free quantitative proteomics data sets to compare imputation strategies"},
    "10.1109/MSEC.2023.3279773": {"venue": "IEEE Security & Privacy"},
    "10.7717/peerj-cs.86": {"authors[-1]": ["FORCE11 Software Citation Working Group", ""]},
    "10.1186/gb-2010-11-8-r86": {"authors[-1]": ["Galaxy Team", ""]},
    "10.1093/bioinformatics/btp352": {"authors[-1]": ["1000 Genome Project Data Processing Subgroup", ""]},
    "10.1038/351290a0": {"authors[2]": ["Stevanović", "Stefan"]},
    "10.1093/nar/13.9.3021": {"title": "Nomenclature for incompletely specified bases in nucleic acid sequences: recommendations 1984"},
    "10.1145/351240.351266": {"title": "QuickCheck: a lightweight tool for random testing of Haskell programs"},
    "10.17487/RFC4180": {"venue": "RFC 4180", "authors": [["Shafranovich", "Y."]]},
    "10.17487/RFC8259": {"venue": "RFC 8259", "authors": [["Bray", "Tim"]]},
    "10.17487/RFC6596": {"venue": "RFC 6596"},
    "10.17487/RFC7322": {"venue": "RFC 7322"},
    "10.17487/RFC9309": {"venue": "RFC 9309"},
    # Crossref drops the accents of two family names.
    "10.1109/ICSE.2019.00122": {"authors[2]": ["Vega-Márquez", "Olga Lucero"], "authors[3]": ["Linares-Vásquez", "Mario"]},
    # Crossref lists the investigator group first.
    "10.1186/s12911-017-0430-8": {"authors": [["Ancker", "Jessica S."], ["Edwards", "Alison"], ["Nosal", "Sarah"], ["Hauser", "Diane"], ["Mauer", "Elizabeth"], ["Kaushal", "Rainu"], ["HITEC Investigators", ""]]},
    # Crossref splits one name as "Thomas August, J." and gives some initials without points.
    "10.1186/s12920-017-0301-2": {"authors[8]": ["August", "J. Thomas"]},
    "10.1186/1471-2105-9-S1-S18": {"authors[1]": ["Heiny", "A. T."], "authors[3]": ["August", "J. Thomas"]},
    # A group author and the publisher, which Crossref records as the institution only.
    "10.59161/AUEZ1291": {"authors": [["Bureau International des Poids et Mesures", ""]], "venue": "BIPM", "title": "The International System of Units (SI), 9th edition"},
}


def clean(text):
    text = html.unescape(re.sub(r"<[^>]+>", "", text or ""))
    text = re.sub(r"\s+", " ", text).replace("‐", "-").replace("( ", "(").replace(" -mers", "-mers")
    return text.strip().rstrip(".")


def fetch(doi):
    req = urllib.request.Request("https://api.crossref.org/works/" + doi, headers={"User-Agent": USER_AGENT})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return json.load(resp)["message"]
        except urllib.error.HTTPError as e:
            if e.code == 404:
                raise
            err = e
        except (urllib.error.URLError, TimeoutError) as e:
            err = e
        time.sleep(2 ** (attempt + 1))
    raise err


def normalise(doi, m):
    authors = [[a.get("family") or a.get("name") or "", re.sub(r"\s+", " ", a.get("given", "")).strip()] for a in m.get("author", [])]
    date = (m.get("published-print") or m.get("published-online") or m.get("issued"))["date-parts"][0]
    rec = {
        "year": date[0],
        "title": clean((m.get("title") or [""])[0]),
        "venue": clean((m.get("container-title") or [""])[0]),
        "volume": m.get("volume"),
        "issue": m.get("issue"),
        "pages": m.get("page") or m.get("article-number"),
        "authors": authors,
    }
    for field, value in CORRECTIONS.get(doi, {}).items():
        idx = re.fullmatch(r"authors\[(-?\d+)\]", field)
        if idx:
            rec["authors"][int(idx.group(1))] = value
        else:
            rec[field] = value
    return rec


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--snapshot", type=Path, default=SNAPSHOT)
    ap.add_argument("--doi", nargs="*", help="check only these DOIs")
    args = ap.parse_args()
    try:
        snap = json.loads(args.snapshot.read_text(encoding="utf-8"))
    except OSError as e:
        print(f"error: cannot read {args.snapshot}: {e}", file=sys.stderr)
        return 2
    dois = args.doi or sorted(snap)
    drift = 0
    for doi in dois:
        if doi not in snap:
            print(f"{doi}: not in snapshot")
            drift += 1
            continue
        try:
            live = normalise(doi, fetch(doi))
        except Exception as e:  # noqa: BLE001 - report and continue with the next DOI
            print(f"{doi}: cannot fetch ({e})")
            drift += 1
            continue
        stored = snap[doi]
        for field in ("year", "title", "venue", "volume", "issue", "pages", "authors"):
            if live.get(field) != stored.get(field):
                print(f"{doi} [{stored['key']}] {field}: stored {stored.get(field)!r} != Crossref {live.get(field)!r}")
                drift += 1
        time.sleep(0.4)
    print(f"{len(dois)} DOIs checked, {drift} difference(s)")
    return 1 if drift else 0


if __name__ == "__main__":
    sys.exit(main())
