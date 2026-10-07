#!/usr/bin/env python3
"""Synthetic benchmark alignments (D81), deterministic (fixed seeds), standard library only.

    make_data.py scale INPUT FACTOR OUT        INPUT's records FACTOR times, each copy with
                                               point substitutions at RATE (default 0.002)
    make_data.py genome LENGTH COUNT OUT        COUNT nucleotide sequences of LENGTH columns
                                               from one consensus, 1 % substitutions, gaps
    make_data.py diverse COUNT LENGTH OUT       COUNT protein sequences of LENGTH columns with
                                               20 % substitutions (a high-diversity case)

OUT is written uncompressed; compress it afterwards (e.g. `zstd -T0`). INPUT may be .bz2/.gz.
Substitutions only replace residues with residues of the same alphabet, so alignment columns,
gaps and the alphabet are preserved.
"""
import argparse
import bz2
import gzip
import random
import sys

DNA = "ACGT"
PROTEIN = "ACDEFGHIKLMNPQRSTVWY"


def read_fasta(path):
    opener = bz2.open if path.endswith(".bz2") else gzip.open if path.endswith(".gz") else open
    with opener(path, "rt") as f:
        header, seq = None, []
        for line in f:
            line = line.rstrip("\r\n")
            if line.startswith(">"):
                if header is not None:
                    yield header, "".join(seq)
                header, seq = line[1:], []
            else:
                seq.append(line.strip())
        if header is not None:
            yield header, "".join(seq)


def mutate(seq, rate, rng, alphabet):
    if rate <= 0:
        return seq
    chars = list(seq)
    for i, c in enumerate(chars):
        if c.upper() in alphabet and rng.random() < rate:
            chars[i] = rng.choice(alphabet)
    return "".join(chars)


def write(out, records):
    with open(out, "w") as f:
        for header, seq in records:
            f.write(f">{header}\n")
            for i in range(0, len(seq), 60):
                f.write(seq[i : i + 60] + "\n")


def scale(args):
    rng = random.Random(1)
    records = list(read_fasta(args.input))
    letters = set("".join(s for _, s in records[:50]).upper()) - set("-.")
    alphabet = DNA if letters <= set(DNA + "NRYKMSWBDHV") else PROTEIN

    def generate():
        for copy in range(args.factor):
            for header, seq in records:
                yield f"{header}|copy{copy}", mutate(seq, args.rate if copy else 0.0, rng, alphabet)

    write(args.out, generate())


def genome(args):
    rng = random.Random(2)
    consensus = "".join(rng.choice(DNA) for _ in range(args.length))

    def generate():
        for i in range(args.count):
            seq = list(mutate(consensus, 0.01, rng, DNA))
            # A few gap runs, as in real alignments.
            for _ in range(rng.randint(0, 3)):
                start = rng.randrange(args.length)
                for j in range(start, min(args.length, start + rng.randint(1, 30))):
                    seq[j] = "-"
            yield f"seq{i}|host{i % 7}|{2000 + i % 25}", "".join(seq)

    write(args.out, generate())


def diverse(args):
    rng = random.Random(3)
    consensus = "".join(rng.choice(PROTEIN) for _ in range(args.length))
    write(args.out, ((f"seq{i}", mutate(consensus, 0.2, rng, PROTEIN)) for i in range(args.count)))


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="command", required=True)
    s = sub.add_parser("scale")
    s.add_argument("input")
    s.add_argument("factor", type=int)
    s.add_argument("out")
    s.add_argument("--rate", type=float, default=0.002)
    g = sub.add_parser("genome")
    g.add_argument("length", type=int)
    g.add_argument("count", type=int)
    g.add_argument("out")
    d = sub.add_parser("diverse")
    d.add_argument("count", type=int)
    d.add_argument("length", type=int)
    d.add_argument("out")
    args = p.parse_args()
    {"scale": scale, "genome": genome, "diverse": diverse}[args.command](args)


if __name__ == "__main__":
    sys.exit(main())
