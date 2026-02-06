/**
 * PDBViewer - 3D Protein Structure Viewer with HCS Highlighting
 *
 * Displays protein structures from PDB files with HCS regions highlighted.
 * Supports both local file upload and RCSB PDB fetching.
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import $3Dmol, { GLViewer } from '3dmol';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { readTextFile } from '@tauri-apps/plugin-fs';
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
  TooltipProvider,
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
} from 'lucide-react';
import type { Position, ChainInfo, PositionMapping } from '@/lib/types';

interface HCSRegion {
  startPosition: number;
  endPosition: number;
  sequence: string;
  indices: number[];
}

interface PDBViewerProps {
  positions: Position[];
  hcsThreshold: number;
}

export function PDBViewer({ positions, hcsThreshold }: PDBViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<GLViewer | null>(null);

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
  const [isSpinning, setIsSpinning] = useState(false);

  // Compute HCS regions from positions
  const hcsRegions = useMemo(() => {
    const regions: HCSRegion[] = [];
    let currentRegion: HCSRegion | null = null;

    positions.forEach((pos) => {
      const indexVariant = pos.diversity_motifs?.find(
        (v) => v.motif_short === 'I' && v.incidence >= hcsThreshold
      );

      if (indexVariant) {
        if (currentRegion) {
          currentRegion.endPosition = pos.position;
          currentRegion.sequence += indexVariant.sequence.slice(-1);
          currentRegion.indices.push(pos.position);
        } else {
          currentRegion = {
            startPosition: pos.position,
            endPosition: pos.position,
            sequence: indexVariant.sequence,
            indices: [pos.position],
          };
        }
      } else {
        if (currentRegion && currentRegion.indices.length > 1) {
          regions.push(currentRegion);
        }
        currentRegion = null;
      }
    });

    if (currentRegion !== null && (currentRegion as HCSRegion).indices.length > 1) {
      regions.push(currentRegion);
    }

    return regions;
  }, [positions, hcsThreshold]);

  // Get consensus sequence from Index motifs for alignment
  const consensusSequence = useMemo(() => {
    return positions
      .map((pos) => {
        const indexVariant = pos.diversity_motifs?.find((v) => v.motif_short === 'I');
        if (indexVariant && indexVariant.sequence.length > 0) {
          // For overlapping k-mers, take the first character
          return pos.position === 0
            ? indexVariant.sequence
            : indexVariant.sequence.slice(-1);
        }
        return 'X';
      })
      .join('');
  }, [positions]);

  // Fetch PDB from RCSB
  const handleFetchPDB = async () => {
    if (!pdbId.trim()) {
      setError('Please enter a PDB ID');
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const data = await invoke<string>('fetch_pdb', { pdbId: pdbId.trim() });
      await loadPDBData(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  };

  // Upload local PDB file
  const handleUploadPDB = async () => {
    try {
      const result = await open({
        filters: [{ name: 'PDB Files', extensions: ['pdb', 'ent'] }],
        multiple: false,
      });

      if (result) {
        setIsLoading(true);
        setError(null);
        const content = await readTextFile(result as string);
        await loadPDBData(content);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  };

  // Load and parse PDB data
  const loadPDBData = async (data: string) => {
    setPdbData(data);

    try {
      const chainInfos = await invoke<ChainInfo[]>('parse_pdb_sequence', {
        pdbContent: data,
      });
      setChains(chainInfos);

      if (chainInfos.length > 0) {
        setSelectedChain(chainInfos[0].chain_id);
        // Auto-compute mapping for first chain
        await computeMapping(chainInfos[0], mappingMode);
      }
    } catch (err) {
      setError(`Failed to parse PDB: ${err}`);
    }
  };

  // Compute position mapping
  const computeMapping = async (chain: ChainInfo, mode: 'direct' | 'auto') => {
    if (mode === 'direct') {
      try {
        const mapping = await invoke<PositionMapping>('create_direct_mapping', {
          msaPositions: positions.map((p) => p.position),
          pdbResidueNumbers: chain.residue_numbers,
          offset: offset,
        });
        setPositionMapping(mapping);
      } catch (err) {
        setError(`Mapping failed: ${err}`);
      }
    } else {
      // Auto-align mode
      try {
        const mapping = await invoke<PositionMapping>('align_sequences', {
          msaSequence: consensusSequence,
          pdbSequence: chain.sequence,
          pdbResidueNumbers: chain.residue_numbers,
        });
        setPositionMapping(mapping);
      } catch (err) {
        setError(`Alignment failed: ${err}`);
      }
    }
  };

  // Re-compute mapping when chain or mode changes
  useEffect(() => {
    if (selectedChain && chains.length > 0) {
      const chain = chains.find((c) => c.chain_id === selectedChain);
      if (chain) {
        computeMapping(chain, mappingMode);
      }
    }
  }, [selectedChain, mappingMode, offset, chains]);

  // Initialize and update 3Dmol viewer
  useEffect(() => {
    if (!containerRef.current || !pdbData) return;

    // Create viewer if not exists
    if (!viewerRef.current) {
      viewerRef.current = $3Dmol.createViewer(containerRef.current, {
        backgroundColor: 'white',
      });
    }

    const viewer = viewerRef.current;
    viewer.removeAllModels();
    viewer.addModel(pdbData, 'pdb');

    // Style entire structure as cartoon (gray)
    viewer.setStyle({}, { cartoon: { color: 'lightgray' } });

    // Highlight HCS regions in green
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

    // Handle resize
    const handleResize = () => {
      viewer.resize();
      viewer.render();
    };
    window.addEventListener('resize', handleResize);

    return () => {
      window.removeEventListener('resize', handleResize);
    };
  }, [pdbData, positionMapping, selectedChain, hcsRegions, isSpinning]);

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
                placeholder="e.g., 6VXX"
                value={pdbId}
                onChange={(e) => setPdbId(e.target.value.toUpperCase())}
                maxLength={4}
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
        </div>
      </div>
    );
  }

  // PDB loaded - show viewer
  return (
    <TooltipProvider>
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
                    ? 'text-green-600'
                    : positionMapping.coverage >= 50
                    ? 'text-yellow-600'
                    : 'text-red-600'
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

        {/* Spacer */}
        <div className="flex-1" />

        {/* Viewer controls */}
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleZoomIn}>
            <ZoomIn className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleZoomOut}>
            <ZoomOut className="h-4 w-4" />
          </Button>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleReset}>
            <RotateCcw className="h-4 w-4" />
          </Button>
          <Button
            variant={isSpinning ? 'secondary' : 'ghost'}
            size="sm"
            className="h-7 text-xs"
            onClick={handleToggleSpin}
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
            setPdbData(null);
            setChains([]);
            setPositionMapping(null);
            setError(null);
          }}
        >
          Load Different
        </Button>
      </div>

      {/* HCS Legend */}
      <div className="flex items-center gap-4 text-xs">
        <div className="flex items-center gap-1">
          <div className="h-3 w-3 rounded bg-green-500" />
          <span>HCS Regions ({hcsRegions.length})</span>
        </div>
        <div className="flex items-center gap-1">
          <div className="h-3 w-3 rounded bg-gray-300" />
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
        className="relative flex-1 min-h-[300px] rounded border bg-white"
        style={{ position: 'relative' }}
      />
    </div>
    </TooltipProvider>
  );
}
