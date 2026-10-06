//! The streaming k-mer counter (D1).
//!
//! One pass over the normalised rows counts, for every position p (1 ≤ p ≤ L − k + 1), each
//! distinct k-mer starting at p, and — with header metadata — the number of sequences per
//! (k-mer, header value). Nothing proportional to sequences × positions is ever stored: memory
//! is Σₚ (distinct k-mers × (k + 25) bytes + metadata cells × ~19 bytes) plus two batches.
//!
//! * **Exact interning.** Each window is hashed with a Karp–Rabin rolling hash modulo the
//!   Mersenne prime 2⁶¹ − 1 (Karp & Rabin 1987), updated in O(1) per position, and looked up in
//!   a per-position table whose candidates are verified byte by byte, so collisions can never
//!   merge different k-mers. For k > 32 a transition cache maps (k-mer id at p − 1, next residue)
//!   to the id at p, which skips the O(k) comparison for repeated transitions.
//! * **Any k.** 1 ≤ k ≤ L; there is no packing into machine words.
//! * **Parallelism.** Positions are split into contiguous shards processed in parallel on the
//!   caller's rayon pool; memory does not grow with the number of threads (each shard owns its
//!   positions), and results do not depend on the thread count or batch size (k-mer ids are
//!   internal; outputs are sorted by count and bytes).

use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};

use hashbrown::{HashMap, HashTable};
use rayon::prelude::*;

use crate::error::{DimaError, Result};

/// The Mersenne prime 2⁶¹ − 1.
const MODULUS: u64 = (1 << 61) - 1;
/// Fixed hash base (any value in [256, M) works; results never depend on it).
const BASE: u64 = 0x1F35_A1C0_9E37_79B9 % MODULUS;
/// Marks "no k-mer" in per-row state.
const NONE: u32 = u32::MAX;
/// Above this k, the transition cache is used.
const TRANSITION_MIN_K: usize = 33;

/// (a · b) mod 2⁶¹ − 1 for a, b < 2⁶¹ − 1.
#[inline]
const fn mulmod(a: u64, b: u64) -> u64 {
    let x = (a as u128) * (b as u128);
    let r = ((x as u64) & MODULUS) + ((x >> 61) as u64);
    let r = (r & MODULUS) + (r >> 61);
    if r >= MODULUS { r - MODULUS } else { r }
}

#[inline]
const fn addmod(a: u64, b: u64) -> u64 {
    let r = a + b;
    if r >= MODULUS { r - MODULUS } else { r }
}

#[inline]
const fn submod(a: u64, b: u64) -> u64 {
    if a >= b { a - b } else { a + MODULUS - b }
}

/// MurmurHash3's 64-bit finaliser: spreads entropy to every bit (hash tables use both the high
/// and the low bits).
#[inline]
const fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// Hasher for `u64` keys (metadata cells, transitions).
#[derive(Clone, Copy, Debug, Default)]
struct MixHasher(u64);

impl Hasher for MixHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = mix(self.0 ^ u64::from(b));
        }
    }

    fn write_u64(&mut self, x: u64) {
        self.0 = mix(self.0 ^ x);
    }
}

type MixMap<V> = HashMap<u64, V, BuildHasherDefault<MixHasher>>;

/// Distinct k-mers (and metadata cells) seen at one position.
#[derive(Debug)]
struct PositionState {
    arena: Vec<u8>,
    counts: Vec<u64>,
    hashes: Vec<u64>,
    table: HashTable<u32>,
    transitions: Option<MixMap<u32>>,
    cells: Option<MixMap<u64>>,
}

impl PositionState {
    fn new(k: usize, strata: usize) -> Self {
        Self {
            arena: Vec::new(),
            counts: Vec::new(),
            hashes: Vec::new(),
            table: HashTable::new(),
            transitions: (k >= TRANSITION_MIN_K).then(MixMap::default),
            cells: (strata > 0).then(MixMap::default),
        }
    }

    /// The id of `kmer` (with Karp–Rabin hash `kr`), inserting it if new.
    fn intern(&mut self, kmer: &[u8], kr: u64) -> Result<u32> {
        let h = mix(kr);
        let k = kmer.len();
        let Self {
            arena,
            counts,
            hashes,
            table,
            ..
        } = self;
        if let Some(&id) = table.find(h, |&id| {
            let i = id as usize;
            hashes[i] == h && &arena[i * k..(i + 1) * k] == kmer
        }) {
            return Ok(id);
        }
        let id = u32::try_from(counts.len())
            .ok()
            .filter(|&id| id != NONE)
            .ok_or(DimaError::Overflow {
                what: "distinct k-mers at one position",
            })?;
        arena.extend_from_slice(kmer);
        counts.push(0);
        hashes.push(h);
        table.insert_unique(h, id, |&id| hashes[id as usize]);
        Ok(id)
    }
}

/// Per-row rolling state of a shard.
#[derive(Clone, Debug, Default)]
struct RowState {
    hash: Vec<u64>,
    excluded: Vec<u32>,
    previous: Vec<u32>,
}

/// A contiguous range of positions owned by one task.
#[derive(Debug)]
struct Shard {
    start: usize,
    positions: Vec<PositionState>,
    scratch: RowState,
}

/// Counts of one position after all input was read.
#[derive(Debug)]
pub struct PositionCounts {
    k: usize,
    arena: Vec<u8>,
    counts: Vec<u64>,
    /// Metadata cells `((id << 32) | value id, count)`, sorted by key.
    cells: Vec<(u64, u64)>,
}

impl PositionCounts {
    /// Support *N*: sequences with a valid k-mer here.
    #[must_use]
    pub fn support(&self) -> u64 {
        self.counts.iter().sum()
    }

    /// Number of distinct k-mers.
    #[must_use]
    pub fn distinct(&self) -> usize {
        self.counts.len()
    }

    /// Ids ordered by count (descending), then k-mer bytes (ascending): the output order.
    #[must_use]
    pub fn ordered_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = (0..self.counts.len() as u32).collect();
        ids.sort_unstable_by(|&a, &b| {
            self.counts[b as usize]
                .cmp(&self.counts[a as usize])
                .then_with(|| self.kmer(a).cmp(self.kmer(b)))
        });
        ids
    }

    /// The k-mer of an id.
    #[must_use]
    pub fn kmer(&self, id: u32) -> &[u8] {
        let i = id as usize;
        &self.arena[i * self.k..(i + 1) * self.k]
    }

    /// The count of an id.
    #[must_use]
    pub fn count(&self, id: u32) -> u64 {
        self.counts[id as usize]
    }

    /// `(value id, count)` metadata cells of a k-mer, ascending by value id.
    pub fn cells(&self, id: u32) -> impl Iterator<Item = (u32, u64)> + '_ {
        let lo = u64::from(id) << 32;
        let start = self.cells.partition_point(|&(key, _)| key < lo);
        let end = self.cells.partition_point(|&(key, _)| key < lo + (1 << 32));
        #[allow(clippy::cast_possible_truncation)] // The low 32 bits are the value id.
        self.cells[start..end]
            .iter()
            .map(|&(key, count)| (key as u32, count))
    }
}

/// The streaming counter.
#[derive(Debug)]
pub struct Engine {
    k: usize,
    width: usize,
    strata: usize,
    shards: Vec<Shard>,
}

impl Engine {
    /// A counter for alignments of `width` columns, k-mers of length `k`, `strata` stratified
    /// header fields, split into shards for `threads` workers.
    ///
    /// # Errors
    /// A usage error when k is 0 or longer than the alignment.
    pub fn new(width: usize, k: usize, strata: usize, threads: usize) -> Result<Self> {
        if k == 0 {
            return Err(DimaError::usage("k-mer length must be at least 1"));
        }
        if k > width {
            return Err(DimaError::usage(format!(
                "k-mer length {k} is longer than the alignment ({width} columns)"
            ))
            .with_hint(format!("choose a k-mer length between 1 and {width}")));
        }
        let positions = width - k + 1;
        let shard_count = positions.min(4 * threads.max(1)).min(width / k).max(1);
        let base = positions / shard_count;
        let extra = positions % shard_count;
        let mut shards = Vec::with_capacity(shard_count);
        let mut start = 0;
        for s in 0..shard_count {
            let len = base + usize::from(s < extra);
            shards.push(Shard {
                start,
                positions: (0..len).map(|_| PositionState::new(k, strata)).collect(),
                scratch: RowState::default(),
            });
            start += len;
        }
        Ok(Self {
            k,
            width,
            strata,
            shards,
        })
    }

    /// Number of positions (L − k + 1).
    #[must_use]
    pub fn positions(&self) -> usize {
        self.width - self.k + 1
    }

    /// Number of shards (exposed for diagnostics and tests).
    #[must_use]
    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// Counts a batch: `rows` holds `records × width` canonical bytes (0 = excluded column);
    /// `values` holds `records × strata` header value ids (empty without metadata). Runs on the
    /// current rayon pool.
    ///
    /// # Errors
    /// [`DimaError::Cancelled`] or [`DimaError::Overflow`].
    pub fn add_batch(
        &mut self,
        rows: &[u8],
        records: usize,
        values: &[u32],
        cancel: &AtomicBool,
    ) -> Result<()> {
        debug_assert_eq!(rows.len(), records * self.width);
        debug_assert!(self.strata == 0 || values.len() == records * self.strata);
        if records == 0 {
            return Ok(());
        }
        let (k, width, strata) = (self.k, self.width, self.strata);
        self.shards
            .par_iter_mut()
            .try_for_each(|shard| shard.count(rows, records, width, k, values, strata, cancel))
    }

    /// Finishes counting and returns the counts of every position, in position order.
    #[must_use]
    pub fn finish(self) -> Vec<PositionCounts> {
        let k = self.k;
        self.shards
            .into_iter()
            .flat_map(|s| s.positions)
            .map(|p| {
                let mut cells: Vec<(u64, u64)> =
                    p.cells.map(|c| c.into_iter().collect()).unwrap_or_default();
                cells.sort_unstable_by_key(|&(key, _)| key);
                PositionCounts {
                    k,
                    arena: p.arena,
                    counts: p.counts,
                    cells,
                }
            })
            .collect()
    }
}

/// B^(k−1) mod M.
fn base_power(k: usize) -> u64 {
    let mut p = 1u64;
    for _ in 1..k {
        p = mulmod(p, BASE);
    }
    p
}

impl Shard {
    #[allow(clippy::too_many_arguments)]
    fn count(
        &mut self,
        rows: &[u8],
        records: usize,
        width: usize,
        k: usize,
        values: &[u32],
        strata: usize,
        cancel: &AtomicBool,
    ) -> Result<()> {
        if self.positions.is_empty() {
            return Ok(());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(DimaError::Cancelled);
        }
        let top = base_power(k);
        let s = self.start;
        let st = &mut self.scratch;
        st.hash.clear();
        st.excluded.clear();
        st.previous.clear();
        st.previous.resize(records, NONE);
        // Initial windows [s, s + k) of every row.
        for r in 0..records {
            let w = &rows[r * width + s..r * width + s + k];
            let mut h = 0u64;
            let mut ex = 0u32;
            for &c in w {
                h = addmod(mulmod(h, BASE), u64::from(c));
                ex += u32::from(c == 0);
            }
            st.hash.push(h);
            st.excluded.push(ex);
        }
        for (offset, pos) in self.positions.iter_mut().enumerate() {
            let p = s + offset;
            if offset > 0 && offset % 64 == 0 && cancel.load(Ordering::Relaxed) {
                return Err(DimaError::Cancelled);
            }
            for r in 0..records {
                let row = &rows[r * width..(r + 1) * width];
                if offset > 0 {
                    let out = row[p - 1];
                    let inn = row[p + k - 1];
                    st.hash[r] = addmod(
                        mulmod(submod(st.hash[r], mulmod(u64::from(out), top)), BASE),
                        u64::from(inn),
                    );
                    st.excluded[r] = st.excluded[r] - u32::from(out == 0) + u32::from(inn == 0);
                }
                if st.excluded[r] != 0 {
                    st.previous[r] = NONE;
                    continue;
                }
                let kmer = &row[p..p + k];
                let prev = st.previous[r];
                let id = match (&mut pos.transitions, prev) {
                    (Some(tr), prev) if prev != NONE => {
                        let key = (u64::from(prev) << 8) | u64::from(kmer[k - 1]);
                        if let Some(&id) = tr.get(&key) {
                            id
                        } else {
                            let id = pos.intern(kmer, st.hash[r])?;
                            pos.transitions.as_mut().map(|t| t.insert(key, id));
                            id
                        }
                    }
                    _ => pos.intern(kmer, st.hash[r])?,
                };
                // Transitions at p key on the id at p − 1; record this one for p + 1.
                st.previous[r] = id;
                pos.counts[id as usize] += 1;
                if let Some(cells) = &mut pos.cells {
                    for &v in &values[r * strata..(r + 1) * strata] {
                        *cells
                            .entry((u64::from(id) << 32) | u64::from(v))
                            .or_insert(0) += 1;
                    }
                }
            }
        }
        // Keep transition keys valid across batches: a row's previous id refers to this batch only.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Naive reference: per position, a map from k-mer to (count, metadata cells).
    type Naive = Vec<BTreeMap<Vec<u8>, (u64, BTreeMap<u32, u64>)>>;

    fn naive(rows: &[Vec<u8>], k: usize, values: &[Vec<u32>]) -> Naive {
        let width = rows[0].len();
        let mut out: Naive = vec![BTreeMap::new(); width - k + 1];
        for (r, row) in rows.iter().enumerate() {
            for p in 0..=width - k {
                let w = &row[p..p + k];
                if w.contains(&0) {
                    continue;
                }
                let e = out[p].entry(w.to_vec()).or_default();
                e.0 += 1;
                for &v in values.get(r).map(Vec::as_slice).unwrap_or(&[]) {
                    *e.1.entry(v).or_default() += 1;
                }
            }
        }
        out
    }

    fn run(
        rows: &[Vec<u8>],
        k: usize,
        values: &[Vec<u32>],
        threads: usize,
        batch: usize,
    ) -> Vec<PositionCounts> {
        let width = rows[0].len();
        let strata = values.first().map_or(0, Vec::len);
        let mut e = Engine::new(width, k, strata, threads).unwrap();
        let cancel = AtomicBool::new(false);
        for (chunk, vals) in rows
            .chunks(batch)
            .zip(values_chunks(values, batch, rows.len()))
        {
            let flat: Vec<u8> = chunk.concat();
            let v: Vec<u32> = vals.concat();
            e.add_batch(&flat, chunk.len(), &v, &cancel).unwrap();
        }
        e.finish()
    }

    fn values_chunks(values: &[Vec<u32>], batch: usize, n: usize) -> Vec<&[Vec<u32>]> {
        if values.is_empty() {
            vec![&[][..]; n.div_ceil(batch)]
        } else {
            values.chunks(batch).collect()
        }
    }

    fn check(rows: &[Vec<u8>], k: usize, values: &[Vec<u32>]) {
        let want = naive(rows, k, values);
        for threads in [1, 2, 7] {
            for batch in [1, 3, rows.len()] {
                let got = run(rows, k, values, threads, batch);
                assert_eq!(got.len(), want.len());
                for (p, (g, w)) in got.iter().zip(&want).enumerate() {
                    let mut gm: BTreeMap<Vec<u8>, (u64, BTreeMap<u32, u64>)> = BTreeMap::new();
                    for id in g.ordered_ids() {
                        let cells = g.cells(id).collect();
                        gm.insert(g.kmer(id).to_vec(), (g.count(id), cells));
                    }
                    assert_eq!(&gm, w, "k={k} threads={threads} batch={batch} position={p}");
                    assert_eq!(g.support(), w.values().map(|v| v.0).sum::<u64>());
                }
            }
        }
    }

    fn rows_from(seqs: &[&str]) -> Vec<Vec<u8>> {
        seqs.iter()
            .map(|s| s.bytes().map(|b| if b == b'-' { 0 } else { b }).collect())
            .collect()
    }

    #[test]
    fn matches_naive_counter() {
        let rows = rows_from(&[
            "ACGTACGTAC",
            "ACGTTCGTAC",
            "AC-TACGTAC",
            "TTTTTTTTTT",
            "ACGTACGTAC",
            "-CGTACGTA-",
        ]);
        for k in 1..=10 {
            check(&rows, k, &[]);
        }
        let values = vec![
            vec![0, 2],
            vec![1, 2],
            vec![0, 3],
            vec![1, 3],
            vec![0, 2],
            vec![1, 2],
        ];
        for k in [1, 3, 10] {
            check(&rows, k, &values);
        }
    }

    #[test]
    fn long_kmers_use_transitions() {
        // k > 32 exercises the transition cache, including after an excluded column.
        let base = "ACGT".repeat(25);
        let mut v = base.clone().into_bytes();
        v[50] = b'-';
        let mut w = base.clone().into_bytes();
        w[70] = b'T';
        let seqs = [
            base.as_str(),
            std::str::from_utf8(&v).unwrap(),
            std::str::from_utf8(&w).unwrap(),
            base.as_str(),
        ];
        let rows = rows_from(&seqs);
        for k in [33, 40, 100] {
            check(&rows, k, &[]);
        }
    }

    #[test]
    fn ordering_and_cells() {
        let rows = rows_from(&["AAA", "CCC", "CCC", "AAA", "GGG"]);
        let values = vec![vec![5], vec![6], vec![6], vec![7], vec![5]];
        let got = run(&rows, 3, &values, 1, 5);
        let p = &got[0];
        let ids = p.ordered_ids();
        let kmers: Vec<&[u8]> = ids.iter().map(|&i| p.kmer(i)).collect();
        assert_eq!(kmers, [&b"AAA"[..], b"CCC", b"GGG"], "ties by bytes");
        assert_eq!(p.cells(ids[0]).collect::<Vec<_>>(), [(5, 1), (7, 1)]);
        assert_eq!(p.cells(ids[1]).collect::<Vec<_>>(), [(6, 2)]);
        assert_eq!(p.distinct(), 3);
    }

    #[test]
    fn hash_arithmetic() {
        assert_eq!(mulmod(MODULUS - 1, MODULUS - 1), 1);
        assert_eq!(submod(0, 1), MODULUS - 1);
        assert_eq!(addmod(MODULUS - 1, 1), 0);
        // Rolling equals direct hashing.
        let s = b"ACGTTGCAAGT";
        let k = 4;
        let direct = |w: &[u8]| {
            w.iter()
                .fold(0u64, |h, &c| addmod(mulmod(h, BASE), u64::from(c)))
        };
        let top = base_power(k);
        let mut h = direct(&s[..k]);
        for p in 1..=s.len() - k {
            h = addmod(
                mulmod(submod(h, mulmod(u64::from(s[p - 1]), top)), BASE),
                u64::from(s[p + k - 1]),
            );
            assert_eq!(h, direct(&s[p..p + k]));
        }
    }

    #[test]
    fn engine_validation_and_sharding() {
        assert_eq!(
            Engine::new(5, 6, 0, 1).unwrap_err().class(),
            crate::error::ExitClass::Usage
        );
        assert_eq!(
            Engine::new(5, 0, 0, 1).unwrap_err().class(),
            crate::error::ExitClass::Usage
        );
        let e = Engine::new(1353, 9, 0, 8).unwrap();
        assert_eq!(e.positions(), 1345);
        assert_eq!(e.shard_count(), 32);
        let e = Engine::new(10, 9, 0, 8).unwrap();
        assert_eq!(e.shard_count(), 1, "shards are at least k long");
        let mut e = Engine::new(4, 2, 0, 1).unwrap();
        let cancel = AtomicBool::new(true);
        assert!(matches!(
            e.add_batch(b"ACGT", 1, &[], &cancel),
            Err(DimaError::Cancelled)
        ));
    }
}
