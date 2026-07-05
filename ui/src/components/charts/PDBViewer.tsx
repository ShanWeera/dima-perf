/**
 * PDBViewer - 3D Protein Structure Viewer with HCS Highlighting
 *
 * Displays protein structures from PDB files with HCS regions highlighted.
 * Supports both local file upload and RCSB PDB fetching.
 */

import { useState, useEffect, useRef, useCallback, useMemo, memo } from 'react';
import $3Dmol, { GLViewer } from '3dmol';
import { open } from '@tauri-apps/plugin-dialog';
import { readTextFile } from '@tauri-apps/plugin-fs';
import { fetchPdb, parsePdbSequence, alignSequences, createDirectMapping } from '@/lib/tauri';
import { computeHCSRegions, type HCSRegion } from '@/lib/hcs';
import { useSettingsStore } from '@/stores/settingsStore';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  Upload,
  Download,
  RotateCcw,
  ZoomIn,
  ZoomOut,
  Loader2,
  AlertCircle,
  CheckCircle2,
  Info,
  ChevronRight,
  ChevronDown,
} from 'lucide-react';
import type { Position, ChainInfo, PositionMapping } from '@/lib/types';
import { FEATURE_CATEGORIES, FEATURE_CATEGORY_ORDER } from '@/lib/features';
import { useFeatureStore } from '@/stores/featureStore';
import { useFeatureMapping } from '@/hooks/useFeatureMapping';
import { useFeatureHighlight3D } from '@/hooks/useFeatureHighlight3D';
import { UniProtStatusIndicator } from '@/components/features/UniProtStatusIndicator';

// Module-level stable empty arrays prevent render churn from inline `?? []`
// fallbacks that create new references on every render cycle. (Fix 2.10)
const EMPTY_FEATURES: never[] = [];
const EMPTY_RESIDUE_NUMBERS: number[] = [];

interface PDBViewerProps {
  positions: Position[];
  hcsThreshold: number;
  /** Pre-computed HCS regions from parent (avoids recomputing inside this component) */
  precomputedHcsRegions?: HCSRegion[];
  /** Index of the HCS region to highlight prominently (from hover or selection in HCSMap) */
  activeHcsRegionIndex?: number | null;
  /** Index of the persistently selected HCS region — camera zooms only on selection, not hover */
  selectedHcsRegionIndex?: number | null;
}

/**
 * Read the container's computed background-color (set via CSS `bg-background` token)
 * and convert the browser's `rgb(r, g, b)` representation to a hex string that
 * 3Dmol.js can accept. Falls back to white/dark gray for unparseable values.
 */
function computedBgToHex(el: HTMLElement): string {
  const raw = getComputedStyle(el).backgroundColor;
  const match = raw.match(/rgba?\(\s*(\d+)[,\s]+(\d+)[,\s]+(\d+)/);
  if (match) {
    return '#' + [match[1], match[2], match[3]]
      .map(n => parseInt(n, 10).toString(16).padStart(2, '0'))
      .join('');
  }
  return '#ffffff';
}

export const PDBViewer = memo(function PDBViewer({ positions, hcsThreshold, precomputedHcsRegions, activeHcsRegionIndex = null, selectedHcsRegionIndex = null }: PDBViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<GLViewer | null>(null);
  // State-based viewer instance for hooks that need to react to viewer
  // creation. Ref mutations don't trigger re-renders, so hooks receiving
  // viewerRef.current miss the initial mount. (Fix 2.9)
  const [viewerInstance, setViewerInstance] = useState<GLViewer | null>(null);
  // Monotonic counter incremented after each main scene rebuild completes.
  // The feature highlight hook depends on this to re-apply overlays AFTER
  // the base scene is rebuilt (fixes effect ordering race in 5.75).
  const [sceneVersion, setSceneVersion] = useState(0);
  const effectiveTheme = useSettingsStore((s) => s.effectiveTheme);

  // State
  const [pdbData, setPdbData] = useState<string | null>(null);
  const [pdbId, setPdbId] = useState('');
  const [chains, setChains] = useState<ChainInfo[]>([]);
  const [selectedChain, setSelectedChain] = useState<string>('');
  const [mappingMode, setMappingMode] = useState<'direct' | 'auto'>('direct');
  const [offset, setOffset] = useState(0);
  const [positionMapping, setPositionMapping] = useState<PositionMapping | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Generation counter to prevent stale PDB fetch responses from overwriting
  // a newer load. Incremented on each fetch/upload attempt.
  const pdbLoadGenRef = useRef(0);
  const [isSpinning, setIsSpinning] = useState(false);

  // Advanced options on the PDB loader screen
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [preloadAccession, setPreloadAccession] = useState('');

  // Feature store — use individual selectors to prevent cascade re-renders (Fix 5.82).
  // Subscribing to the whole store causes PDBViewer to re-render on every
  // hoveredFeature change from the FeatureTrackViewer panel.
  const uniprotInfo = useFeatureStore((s) => s.uniprotInfo);
  const featureIsLoading = useFeatureStore((s) => s.isLoading);
  const featureError = useFeatureStore((s) => s.error);
  const hoveredFeature = useFeatureStore((s) => s.hoveredFeature);
  const selectedFeature = useFeatureStore((s) => s.selectedFeature);
  const visibleCategories = useFeatureStore((s) => s.visibleCategories);
  const setMappedFeatures = useFeatureStore((s) => s.setMappedFeatures);
  const clearFeatures = useFeatureStore((s) => s.clearFeatures);
  const fetchFeaturesByAccession = useFeatureStore((s) => s.fetchFeaturesByAccession);
  const fetchFeaturesFromPdb = useFeatureStore((s) => s.fetchFeaturesFromPdb);
  const [showFeatures, setShowFeatures] = useState(true);

  // Map UniProt features to MSA coordinates by chaining through PDB alignment.
  // This runs once when UniProt data + PDB chain + MSA mapping are all available.
  const selectedChainInfo = chains.find((c) => c.chain_id === selectedChain);

  const msaToPdb = useMemo(
    () => positionMapping?.msa_to_pdb ?? null,
    [positionMapping]
  );

  // Stabilize fallback values for useFeatureMapping inputs. (Fix 2.10)
  // Inline `?? []` creates a new array reference on every render when the
  // upstream data is null/undefined, causing useFeatureMapping's effect to
  // fire continuously ([] !== []) and trigger render churn.
  const featuresList = useMemo(
    () => uniprotInfo?.features ?? EMPTY_FEATURES,
    [uniprotInfo?.features]
  );
  const uniprotSequence = uniprotInfo?.sequence ?? '';
  const pdbSequence = selectedChainInfo?.sequence ?? '';
  const pdbResidueNumbers = useMemo(
    () => selectedChainInfo?.residue_numbers ?? EMPTY_RESIDUE_NUMBERS,
    [selectedChainInfo?.residue_numbers]
  );

  const { mappedFeatures } = useFeatureMapping({
    features: featuresList,
    uniprotSequence,
    pdbSequence,
    pdbResidueNumbers,
    msaToPdb,
  });

  // Push mapped features to the store so other components (Feature Track panel, HCS Map) can use them
  useEffect(() => {
    setMappedFeatures(mappedFeatures);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- store actions are stable refs
  }, [mappedFeatures]);

  // Compute the active feature (hover takes priority over selection)
  const activeFeature = hoveredFeature ?? selectedFeature ?? null;

  // Use the canonical HCS computation from lib/hcs.ts to ensure consistency
  // Use pre-computed regions from parent when available (avoids redundant O(n) walk).
  // Falls back to local computation for backward compatibility if prop not provided.
  const hcsRegions = useMemo(
    () => precomputedHcsRegions ?? computeHCSRegions(positions, hcsThreshold),
    [precomputedHcsRegions, positions, hcsThreshold]
  );

  // 3D feature highlight overlay (cheap incremental effect, separate from scene rebuild).
  // Uses viewerInstance (state) instead of viewerRef.current (ref) so the hook
  // re-runs when the viewer is created. (Fix 2.9)
  useFeatureHighlight3D({
    viewer: viewerInstance,
    selectedChain,
    msaToPdb,
    mappedFeatures,
    visibleCategories,
    activeFeature,
    selectedFeature,
    showFeatures,
    hcsRegions,
    sceneVersion,
  });

  // Build a 1-residue-per-position consensus for auto-alignment (Fix 5.6).
  //
  // CRITICAL: The old approach built a stitched k-mer string (k + n - 1 chars),
  // where alignment keys were character indices — a DIFFERENT coordinate space
  // from position numbers used by HCS highlighting. For k > 1, this mismatch
  // caused wrong residues to be highlighted.
  //
  // New approach: One character per MSA position, taken from the middle of each
  // position's Index motif (the column-representative residue). This ensures
  // alignment key `i` directly corresponds to position `i`, matching HCS lookup.
  const consensusSequence = useMemo(() => {
    if (positions.length === 0) return '';
    const chars: string[] = [];
    for (const pos of positions) {
      const indexVariant = pos.diversity_motifs?.find((v) => v.motif_short === 'I');
      if (indexVariant && indexVariant.sequence.length > 0) {
        // Use the middle character of the k-mer as the column representative.
        // This is the most biologically meaningful residue for alignment:
        // it represents the "center" of the k-mer window at this position.
        const midIdx = Math.floor(indexVariant.sequence.length / 2);
        chars.push(indexVariant.sequence[midIdx]);
      } else {
        chars.push('X');
      }
    }
    return chars.join('');
  }, [positions]);

  // Fetch PDB from RCSB with stale-response guard
  const handleFetchPDB = async () => {
    if (!pdbId.trim()) {
      setError('Please enter a PDB ID');
      return;
    }

    const thisGen = ++pdbLoadGenRef.current;
    setIsLoading(true);
    setError(null);

    try {
      const trimmedId = pdbId.trim();
      const data = await fetchPdb(trimmedId);
      // Discard if a newer fetch/upload started while this was in-flight
      if (pdbLoadGenRef.current !== thisGen) return;
      await loadPDBData(data, trimmedId, preloadAccession.trim());
    } catch (err) {
      if (pdbLoadGenRef.current !== thisGen) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (pdbLoadGenRef.current === thisGen) setIsLoading(false);
    }
  };

  // Upload local PDB file
  const handleUploadPDB = async () => {
    const thisGen = ++pdbLoadGenRef.current;
    try {
      const result = await open({
        filters: [{ name: 'PDB Files', extensions: ['pdb', 'ent'] }],
        multiple: false,
      });

      if (result) {
        setIsLoading(true);
        setError(null);
        const content = await readTextFile(result as string);
        if (pdbLoadGenRef.current !== thisGen) return;
        await loadPDBData(content);
      }
    } catch (err) {
      if (pdbLoadGenRef.current !== thisGen) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      if (pdbLoadGenRef.current === thisGen) setIsLoading(false);
    }
  };

  // Load and parse PDB data, then auto-detect UniProt features.
  // Both pdbId and accession are passed as parameters to avoid stale closure
  // issues — React state values captured at call time may differ from the
  // state values at the time the async body executes.
  const loadPDBData = async (data: string, fetchedPdbId?: string, accession?: string) => {
    // Clear stale feature data from previous PDB before loading new one
    clearFeatures();
    setPdbData(data);

    try {
      const chainInfos = await parsePdbSequence(data);
      setChains(chainInfos);

      if (chainInfos.length > 0) {
        setSelectedChain(chainInfos[0].chain_id);
        // Auto-compute mapping for first chain
        await computeMapping(chainInfos[0], mappingMode);
      }

      // Auto-detect UniProt features in the background (non-blocking).
      // Uses the passed parameters to avoid stale closure issues.
      // If the user pre-filled an accession, use that directly; otherwise
      // try to look it up via the RCSB Data API from the PDB ID.
      const currentPdbId = fetchedPdbId ?? '';
      const currentAccession = accession ?? '';
      if (currentAccession) {
        fetchFeaturesByAccession(currentAccession);
      } else if (currentPdbId) {
        // Entity 1 is the most common; the chain selector can refine later
        fetchFeaturesFromPdb(currentPdbId, 1);
      }
      // For local uploads without a PDB ID or accession, auto-detect is
      // not possible. The UniProtStatusIndicator will show a prompt to
      // enter an accession manually.
    } catch (err) {
      setError(`Failed to parse PDB: ${err}`);
    }
  };

  // Compute position mapping
  const computeMapping = async (chain: ChainInfo, mode: 'direct' | 'auto') => {
    if (mode === 'direct') {
      try {
        const mapping = await createDirectMapping(
          positions.map((p) => p.position),
          chain.residue_numbers,
          offset,
        );
        setPositionMapping(mapping);
      } catch (err) {
        setPositionMapping(null);
        setError(`Mapping failed: ${err}`);
      }
    } else {
      // Auto-align mode
      try {
        const mapping = await alignSequences(
          consensusSequence,
          chain.sequence,
          chain.residue_numbers,
        );
        setPositionMapping(mapping);
      } catch (err) {
        setPositionMapping(null);
        setError(`Alignment failed: ${err}`);
      }
    }
  };

  // Re-compute mapping when chain, mode, or input data changes.
  // `positions` is included so that loading new analysis results triggers a remap.
  useEffect(() => {
    if (selectedChain && chains.length > 0) {
      const chain = chains.find((c) => c.chain_id === selectedChain);
      if (chain) {
        computeMapping(chain, mappingMode);
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- computeMapping uses refs internally
  }, [selectedChain, mappingMode, offset, chains, positions]);

  // Destroy the 3Dmol WebGL viewer on component unmount to free GPU resources.
  // NOTE: Do NOT call clearFeatures() here — panel hide/unmount should not destroy
  // cross-panel scientific context (FeatureTrackViewer/HCSMap may still be visible).
  // Features are cleared only on explicit user actions (Load Different, new PDB). (Fix 5.77)
  useEffect(() => {
    return () => {
      if (viewerRef.current) {
        viewerRef.current.spin(false);
        viewerRef.current.removeAllModels();
        viewerRef.current.clear();
        viewerRef.current = null;
      }
    };
  }, []);

  // Update 3Dmol background color when theme changes.
  // Derives color from the container's computed `backgroundColor` (which uses the
  // CSS `bg-background` token) so it stays in sync with the design system. (Fix 9.1.5)
  useEffect(() => {
    if (viewerRef.current && containerRef.current) {
      const bgHex = computedBgToHex(containerRef.current);
      viewerRef.current.setBackgroundColor(bgHex);
      viewerRef.current.render();
    }
  }, [effectiveTheme]);

  // Main scene effect: loads/rebuilds the 3D model and applies base styling.
  // Does NOT depend on activeHcsRegionIndex/selectedHcsRegionIndex to avoid
  // costly full-model rebuilds on every HCS hover. (Fix 5.51)
  useEffect(() => {
    if (!containerRef.current || !pdbData) return;

    if (!viewerRef.current) {
      viewerRef.current = $3Dmol.createViewer(containerRef.current, {
        backgroundColor: computedBgToHex(containerRef.current),
      });
      setViewerInstance(viewerRef.current);
    }

    const viewer = viewerRef.current;
    viewer.removeAllModels();
    viewer.addModel(pdbData, 'pdb');

    // Style entire structure as cartoon (gray)
    viewer.setStyle({}, { cartoon: { color: 'lightgray' } });

    // Highlight all HCS regions in green (base coloring)
    if (positionMapping && selectedChain) {
      hcsRegions.forEach((region) => {
        const pdbResidues: number[] = [];
        region.indices.forEach((msaPos) => {
          const pdbResi = positionMapping.msa_to_pdb[msaPos];
          if (pdbResi !== undefined) {
            pdbResidues.push(pdbResi);
          }
        });

        if (pdbResidues.length > 0) {
          viewer.setStyle(
            { chain: selectedChain, resi: pdbResidues },
            {
              cartoon: { color: 'green' },
              stick: { color: 'green', radius: 0.15 },
            }
          );
        }
      });
    }

    viewer.zoomTo();
    viewer.render();

    if (isSpinning) {
      viewer.spin('y', 1);
    } else {
      viewer.spin(false);
    }

    // Signal that the base scene has been rebuilt so the feature highlight
    // hook re-applies its overlay on top of the fresh scene. (Fix 5.75)
    setSceneVersion((v) => v + 1);

    // Observe container resizes
    const container = containerRef.current;
    let resizeObserver: ResizeObserver | null = null;
    if (container) {
      resizeObserver = new ResizeObserver(() => {
        viewer.resize();
        viewer.render();
      });
      resizeObserver.observe(container);
    }

    return () => {
      resizeObserver?.disconnect();
    };
  }, [pdbData, positionMapping, selectedChain, hcsRegions, isSpinning]);

  // Incremental HCS highlight effect: applies gold highlighting to the active
  // region WITHOUT rebuilding the model. Only uses setStyle + render. (Fix 5.51)
  useEffect(() => {
    if (!viewerRef.current || !positionMapping || !selectedChain) return;
    const viewer = viewerRef.current;

    // First, restore all HCS regions to base green (in case previous highlight)
    hcsRegions.forEach((region) => {
      const pdbResidues: number[] = [];
      region.indices.forEach((msaPos) => {
        const pdbResi = positionMapping.msa_to_pdb[msaPos];
        if (pdbResi !== undefined) pdbResidues.push(pdbResi);
      });
      if (pdbResidues.length > 0) {
        viewer.setStyle(
          { chain: selectedChain, resi: pdbResidues },
          { cartoon: { color: 'green' }, stick: { color: 'green', radius: 0.15 } }
        );
      }
    });

    // Apply gold highlight to the active region
    if (
      activeHcsRegionIndex !== null &&
      activeHcsRegionIndex >= 0 &&
      activeHcsRegionIndex < hcsRegions.length
    ) {
      const activeRegion = hcsRegions[activeHcsRegionIndex];
      const activePdbResidues: number[] = [];
      activeRegion.indices.forEach((msaPos) => {
        const pdbResi = positionMapping.msa_to_pdb[msaPos];
        if (pdbResi !== undefined) activePdbResidues.push(pdbResi);
      });

      if (activePdbResidues.length > 0) {
        viewer.setStyle(
          { chain: selectedChain, resi: activePdbResidues },
          { cartoon: { color: '#FFD700' }, stick: { color: '#FFD700', radius: 0.25 } }
        );
        // Camera zoom only on persistent selection (click), not hover
        if (selectedHcsRegionIndex === activeHcsRegionIndex) {
          viewer.zoomTo({ chain: selectedChain, resi: activePdbResidues });
        }
      }
    }

    viewer.render();
  }, [activeHcsRegionIndex, selectedHcsRegionIndex, hcsRegions, positionMapping, selectedChain]);

  // Viewer controls
  const handleZoomIn = useCallback(() => {
    viewerRef.current?.zoom(1.2, 300);
  }, []);

  const handleZoomOut = useCallback(() => {
    viewerRef.current?.zoom(0.8, 300);
  }, []);

  const handleReset = useCallback(() => {
    viewerRef.current?.zoomTo({}, 300);
  }, []);

  const handleToggleSpin = useCallback(() => {
    setIsSpinning((prev) => !prev);
  }, []);

  // No PDB loaded - show input UI
  if (!pdbData) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-4 overflow-auto p-4">
        <div className="text-center">
          <h3 className="text-base font-medium">Load a PDB Structure</h3>
          <p className="mt-1 text-xs text-muted-foreground">
            Upload a local PDB file or fetch from RCSB PDB
          </p>
        </div>

        {error && (
          <div className="flex items-center gap-2 text-xs text-destructive">
            <AlertCircle className="h-3 w-3 shrink-0" />
            <span className="break-words">{error}</span>
          </div>
        )}

        <div className="flex w-full max-w-xs flex-col gap-3">
          {/* Fetch from RCSB */}
          <div className="space-y-1.5">
            <Label className="text-xs">Fetch from RCSB PDB</Label>
            <div className="flex gap-2">
              <Input
                placeholder="e.g., 6VXX or pdb_00001abc"
                value={pdbId}
                onChange={(e) => setPdbId(e.target.value.toUpperCase())}
                maxLength={12}
                className="h-8 uppercase text-sm"
              />
              <Button size="sm" onClick={handleFetchPDB} disabled={isLoading}>
                {isLoading ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Download className="h-4 w-4" />
                )}
              </Button>
            </div>
          </div>

          <div className="relative py-2">
            <div className="absolute inset-0 flex items-center">
              <span className="w-full border-t" />
            </div>
            <div className="relative flex justify-center text-xs uppercase">
              <span className="bg-card px-2 text-muted-foreground">or</span>
            </div>
          </div>

          {/* Upload local file */}
          <Button variant="outline" size="sm" onClick={handleUploadPDB} disabled={isLoading}>
            <Upload className="mr-2 h-4 w-4" />
            Upload PDB File
          </Button>

          {/* Advanced options: UniProt accession (progressive disclosure) */}
          <button
            onClick={() => setShowAdvanced(!showAdvanced)}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors mt-1"
            aria-expanded={showAdvanced}
          >
            {showAdvanced ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
            Advanced options
          </button>
          {showAdvanced && (
            <div className="space-y-1.5 rounded-md border p-3 transition-all duration-200">
              <Label className="text-xs">UniProt Accession (optional)</Label>
              <Input
                value={preloadAccession}
                onChange={(e) => setPreloadAccession(e.target.value.toUpperCase())}
                placeholder="e.g. P0DTC2"
                className="h-8 text-sm"
              />
              <p className="text-xs text-muted-foreground">
                Provide a UniProt accession to load protein feature annotations.
                If left blank, it will be auto-detected from the PDB.
              </p>
            </div>
          )}
        </div>
      </div>
    );
  }

  // PDB loaded - show viewer
  return (
    <div className="flex h-full flex-col gap-2">
      {/* Controls bar */}
      <div className="flex flex-wrap items-center gap-4 border-b pb-2">
        {/* Chain selector */}
        <div className="flex items-center gap-2">
          <Label className="text-xs">Chain:</Label>
          <Select value={selectedChain} onValueChange={setSelectedChain}>
            <SelectTrigger className="h-7 w-20">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {chains.map((chain) => (
                <SelectItem key={chain.chain_id} value={chain.chain_id}>
                  {chain.chain_id} ({chain.sequence.length} res)
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        {/* Mapping mode */}
        <div className="flex items-center gap-2">
          <Label className="text-xs">Auto-align:</Label>
          <Switch
            id="pdb-mapping-mode"
            aria-label="Toggle auto-align mapping mode"
            checked={mappingMode === 'auto'}
            onCheckedChange={(checked) =>
              setMappingMode(checked ? 'auto' : 'direct')
            }
          />
        </div>

        {/* Offset (only for direct mapping) */}
        {mappingMode === 'direct' && (
          <div className="flex items-center gap-2">
            <Label className="text-xs">Offset:</Label>
            <Input
              type="number"
              value={offset}
              onChange={(e) => setOffset(parseInt(e.target.value) || 0)}
              className="h-7 w-16"
            />
          </div>
        )}

        {/* Mapping status */}
        {positionMapping && (
          <Tooltip>
            <TooltipTrigger asChild>
              <div
                className={`flex items-center gap-1 text-xs ${
                  positionMapping.coverage >= 80
                    ? 'text-green-600 dark:text-green-400'
                    : positionMapping.coverage >= 50
                    ? 'text-yellow-600 dark:text-yellow-400'
                    : 'text-red-600 dark:text-red-400'
                }`}
              >
                {positionMapping.coverage >= 80 ? (
                  <CheckCircle2 className="h-3 w-3" />
                ) : (
                  <AlertCircle className="h-3 w-3" />
                )}
                {positionMapping.coverage.toFixed(1)}% mapped
              </div>
            </TooltipTrigger>
            <TooltipContent>
              <p>Coverage: {positionMapping.coverage.toFixed(1)}%</p>
              <p>
                {Object.keys(positionMapping.msa_to_pdb).length} of{' '}
                {positions.length} positions mapped
              </p>
              {positionMapping.coverage < 80 && (
                <p className="text-yellow-500">
                  Try adjusting offset or use auto-align
                </p>
              )}
            </TooltipContent>
          </Tooltip>
        )}

        {/* UniProt feature status */}
        <UniProtStatusIndicator
          uniprotInfo={uniprotInfo}
          isLoading={featureIsLoading}
          error={featureError}
          onManualFetch={(acc) => fetchFeaturesByAccession(acc)}
        />

        {/* Feature overlay toggle (only when features are loaded) */}
        {mappedFeatures.length > 0 && (
          <div className="flex items-center gap-2">
            <Label className="text-xs">Features:</Label>
            <Switch
              id="pdb-show-features"
              aria-label="Toggle feature overlay display"
              checked={showFeatures}
              onCheckedChange={setShowFeatures}
            />
          </div>
        )}

        {/* Spacer */}
        <div className="flex-1" />

        {/* Viewer controls */}
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleZoomIn} aria-label="Zoom in">
            <ZoomIn className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleZoomOut} aria-label="Zoom out">
            <ZoomOut className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleReset} aria-label="Reset view">
            <RotateCcw className="h-4 w-4" />
          </Button>
          <Button
            variant={isSpinning ? 'secondary' : 'ghost'}
            size="sm"
            className="h-7 text-xs"
            onClick={handleToggleSpin}
            aria-pressed={isSpinning}
            aria-label={isSpinning ? 'Stop spinning' : 'Start spinning'}
          >
            Spin
          </Button>
        </div>

        {/* Load different PDB */}
        <Button
          variant="outline"
          size="sm"
          className="h-7 text-xs"
          onClick={() => {
            // Destroy the existing WebGL viewer before nullifying pdbData.
            // If we don't, viewerRef keeps a reference to the old (soon-unmounted)
            // viewer, causing the next load to skip createViewer (stale context bug).
            if (viewerRef.current) {
              viewerRef.current.spin(false);
              viewerRef.current.removeAllModels();
              viewerRef.current.clear();
              viewerRef.current = null;
              setViewerInstance(null);
            }
            setPdbData(null);
            setChains([]);
            setPositionMapping(null);
            setError(null);
            clearFeatures();
            setPreloadAccession('');
            setShowAdvanced(false);
          }}
        >
          Load Different
        </Button>
      </div>

      {/* Post-load error banner — mapping/alignment/parse errors that occur AFTER PDB data is loaded */}
      {error && (
        <div className="flex items-center gap-2 rounded-md border border-destructive/50 bg-destructive/10 px-3 py-1.5 text-xs text-destructive">
          <AlertCircle className="h-3.5 w-3.5 shrink-0" />
          <span className="break-words">{error}</span>
          <button
            onClick={() => setError(null)}
            className="ml-auto shrink-0 text-destructive/80 hover:text-destructive"
            aria-label="Dismiss error"
          >
            ×
          </button>
        </div>
      )}

      {/* HCS + Features Legend */}
      <div className="flex flex-wrap items-center gap-3 text-xs">
        <div className="flex items-center gap-1">
          <div className="h-3 w-3 rounded bg-green-500" />
          <span>HCS Regions ({hcsRegions.length})</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="h-3 w-3 rounded" style={{ backgroundColor: '#FFD700' }} />
          <span>Active HCS</span>
        </div>
        {/* Feature category swatches (only visible categories with features) */}
        {showFeatures && mappedFeatures.length > 0 && (
          <>
            {FEATURE_CATEGORY_ORDER.filter(
              (key) =>
                visibleCategories.has(key) &&
                mappedFeatures.some((f) => f.categoryKey === key)
            ).map((key) => {
              const config = FEATURE_CATEGORIES[key];
              return (
                <div key={key} className="flex items-center gap-1">
                  <div className="h-3 w-3 rounded" style={{ backgroundColor: config.color }} />
                  <span>{config.label}</span>
                </div>
              );
            })}
          </>
        )}
        <div className="flex items-center gap-1">
          <div className="h-3 w-3 rounded bg-muted" />
          <span>Other</span>
        </div>
        {hcsRegions.length > 0 && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button variant="ghost" size="icon" className="h-5 w-5">
                <Info className="h-3 w-3" />
              </Button>
            </TooltipTrigger>
            <TooltipContent className="max-w-xs">
              <p className="font-medium">HCS Regions:</p>
              <ul className="mt-1 space-y-0.5 text-xs">
                {hcsRegions.slice(0, 5).map((region, i) => (
                  <li key={i}>
                    Positions {region.startPosition}-{region.endPosition}
                  </li>
                ))}
                {hcsRegions.length > 5 && (
                  <li>...and {hcsRegions.length - 5} more</li>
                )}
              </ul>
            </TooltipContent>
          </Tooltip>
        )}
      </div>

      {/* 3Dmol viewer container */}
      <div
        ref={containerRef}
        className="relative flex-1 min-h-[300px] rounded border bg-background"
        style={{ position: 'relative' }}
      />
    </div>
  );
});
