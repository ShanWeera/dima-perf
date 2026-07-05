/**
 * useFeatureMapping - Maps UniProt feature positions to MSA coordinates.
 *
 * Chains the alignment: UniProt → PDB → MSA.
 * 1. Aligns the UniProt canonical sequence against the PDB chain sequence
 *    (via the existing `align_sequences` Tauri command) to build UniProt→PDB map.
 * 2. Inverts the existing MSA→PDB map to build PDB→MSA.
 * 3. Composes both to translate each feature's UniProt start/end into MSA positions.
 *
 * Returns MappedFeature[] (features with msaBegin/msaEnd filled in).
 */

import { useEffect, useState, useRef } from 'react';
import type { ProteinFeature, MappedFeature, PositionMapping } from '@/lib/types';
import { categorizeFeature } from '@/lib/features';
import { alignSequences } from '@/lib/tauri';

interface UseFeatureMappingArgs {
  /** Raw features from UniProt */
  features: ProteinFeature[];
  /** Full UniProt canonical sequence */
  uniprotSequence: string;
  /** PDB chain sequence */
  pdbSequence: string;
  /** PDB residue numbers corresponding to pdbSequence */
  pdbResidueNumbers: number[];
  /** Existing MSA→PDB mapping (from the PDB viewer) */
  msaToPdb: Record<number, number> | null;
}

export function useFeatureMapping({
  features,
  uniprotSequence,
  pdbSequence,
  pdbResidueNumbers,
  msaToPdb,
}: UseFeatureMappingArgs): {
  mappedFeatures: MappedFeature[];
  isMapping: boolean;
  mappingError: string | null;
} {
  const [mappedFeatures, setMappedFeatures] = useState<MappedFeature[]>([]);
  const [isMapping, setIsMapping] = useState(false);
  const [mappingError, setMappingError] = useState<string | null>(null);

  // Monotonic request ID eliminates the boolean-abort race: a new effect
  // increments the counter, and only the latest invocation's ID matches
  // the ref when the async work completes. A shared boolean can be
  // erroneously cleared by a subsequent effect invocation.
  const requestIdRef = useRef(0);

  useEffect(() => {
    if (
      features.length === 0 ||
      !uniprotSequence ||
      !pdbSequence ||
      pdbResidueNumbers.length === 0 ||
      !msaToPdb
    ) {
      setMappedFeatures([]);
      setIsMapping(false);
      setMappingError(null);
      return;
    }

    const thisRequestId = ++requestIdRef.current;
    setIsMapping(true);
    setMappingError(null);

    const isStale = () => requestIdRef.current !== thisRequestId;

    (async () => {
      try {
        const uniprotToPdbMapping: PositionMapping = await alignSequences(
          uniprotSequence,
          pdbSequence,
          pdbResidueNumbers
        );

        if (isStale()) return;

        // Invert msaToPdb to get pdbToMsa. Uses a Map<number, number[]> to handle
        // the one-to-many case where multiple MSA positions map to the same PDB
        // residue (e.g., overlapping k-mer windows or alignment ambiguity).
        // The FIRST (lowest) MSA position is used for the feature's start, and
        // the LAST (highest) for the end — giving the widest correct extent.
        const pdbToMsaMulti = new Map<number, number[]>();
        for (const [msaPos, pdbResi] of Object.entries(msaToPdb)) {
          const msaNum = Number(msaPos);
          const existing = pdbToMsaMulti.get(pdbResi);
          if (existing) {
            existing.push(msaNum);
          } else {
            pdbToMsaMulti.set(pdbResi, [msaNum]);
          }
        }
        // Sort each entry's MSA positions so [0] is min and [n-1] is max
        for (const positions of pdbToMsaMulti.values()) {
          positions.sort((a, b) => a - b);
        }

        // Helper: map a UniProt position through the chain UniProt→PDB→MSA
        const mapUniprotToMsa = (uniprotPos: number, preferFirst: boolean): number | null => {
          const pdbResi = uniprotToPdbMapping.msa_to_pdb[uniprotPos];
          if (pdbResi === undefined) return null;
          const msaPositions = pdbToMsaMulti.get(pdbResi);
          if (!msaPositions || msaPositions.length === 0) return null;
          return preferFirst ? msaPositions[0] : msaPositions[msaPositions.length - 1];
        };

        // Compose UniProt→PDB→MSA for each feature.
        // Maps ALL residues in the feature range (not just endpoints) to detect
        // internal alignment gaps. If intermediate positions map to MSA, we use
        // the true extent; otherwise the feature bar may cover unmapped columns.
        const mapped: MappedFeature[] = features
          .map((f) => {
            const categoryKey = categorizeFeature(f);
            if (!categoryKey) return null;

            // Map all positions in the feature's UniProt range to find the true
            // MSA extent, handling cases where begin/end don't map but interior does.
            let msaMin: number | null = null;
            let msaMax: number | null = null;
            for (let uniPos = f.begin; uniPos <= f.end; uniPos++) {
              const msaFirst = mapUniprotToMsa(uniPos, true);
              const msaLast = mapUniprotToMsa(uniPos, false);
              if (msaFirst !== null) {
                msaMin = msaMin === null ? msaFirst : Math.min(msaMin, msaFirst);
              }
              if (msaLast !== null) {
                msaMax = msaMax === null ? msaLast : Math.max(msaMax, msaLast);
              }
            }

            return {
              ...f,
              msaBegin: msaMin,
              msaEnd: msaMax,
              categoryKey,
            } as MappedFeature;
          })
          .filter((f): f is MappedFeature => f !== null);

        if (!isStale()) {
          setMappedFeatures(mapped);
        }
      } catch (err) {
        if (!isStale()) {
          setMappingError(
            err instanceof Error ? err.message : String(err)
          );
          setMappedFeatures([]);
        }
      } finally {
        if (!isStale()) {
          setIsMapping(false);
        }
      }
    })();

    // No explicit cleanup needed — incrementing requestIdRef invalidates
    // the stale invocation via the isStale() check.
  }, [features, uniprotSequence, pdbSequence, pdbResidueNumbers, msaToPdb]);

  return { mappedFeatures, isMapping, mappingError };
}
