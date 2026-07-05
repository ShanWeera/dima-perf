/**
 * DiMA Desktop - Filter Panel
 * 
 * Advanced search and filter panel for positions and variants.
 */

import { useState, useMemo, memo } from 'react';
import { Search, X, Save, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { SearchFilters, FilterPreset, Position, MotifType } from '@/lib/types';
import { MOTIF_COLORS } from '@/lib/colors';
import { 
  DEFAULT_FILTERS, 
  countActiveFilters, 
  getEntropyRange, 
  getPositionRange 
} from '@/lib/filters';

interface FilterPanelProps {
  positions: Position[];
  filters: SearchFilters;
  onFiltersChange: (filters: SearchFilters) => void;
  presets: FilterPreset[];
  onSavePreset: (name: string) => void;
  onLoadPreset: (preset: FilterPreset) => void;
  onDeletePreset: (id: string) => void;
}

const MOTIF_OPTIONS: { value: MotifType; label: string; color: string }[] = [
  { value: 'I', label: 'Index', color: MOTIF_COLORS['I'] },
  { value: 'Ma', label: 'Major', color: MOTIF_COLORS['Ma'] },
  { value: 'Mi', label: 'Minor', color: MOTIF_COLORS['Mi'] },
  { value: 'U', label: 'Unique', color: MOTIF_COLORS['U'] },
];

export const FilterPanel = memo(function FilterPanel({
  positions,
  filters,
  onFiltersChange,
  presets,
  onSavePreset,
  onLoadPreset,
  onDeletePreset,
}: FilterPanelProps) {
  const [presetName, setPresetName] = useState('');
  const [showPresetDialog, setShowPresetDialog] = useState(false);

  // Get min/max values for sliders
  const { minPos, maxPos, maxEntropy } = useMemo(() => {
    const [minP, maxP] = getPositionRange(positions);
    const [, maxE] = getEntropyRange(positions);
    return {
      minPos: minP,
      maxPos: maxP,
      maxEntropy: maxE,
    };
  }, [positions]);

  // Track active filter count for UI feedback
  const activeFilterCount = useMemo(() => countActiveFilters(filters), [filters]);

  const handleReset = () => {
    onFiltersChange({ ...DEFAULT_FILTERS });
  };

  const handlePositionRangeChange = (start: number | null, end: number | null) => {
    // Only apply a range filter when at least one bound is explicitly set and finite.
    // Treat null as "no bound" rather than coercing to min/max — prevents users from
    // accidentally filtering when they only typed in one field. (Fix 5.24)
    const validStart = start !== null && Number.isFinite(start) ? Math.max(start, minPos) : null;
    const validEnd = end !== null && Number.isFinite(end) ? Math.min(end, maxPos) : null;

    if (validStart === null && validEnd === null) {
      onFiltersChange({ ...filters, positionRange: null });
    } else {
      onFiltersChange({
        ...filters,
        positionRange: [validStart ?? minPos, validEnd ?? maxPos],
      });
    }
  };

  const handleEntropyRangeChange = (min: number | null, max: number | null) => {
    const validMin = min !== null && Number.isFinite(min) ? Math.max(min, 0) : null;
    const validMax = max !== null && Number.isFinite(max) ? Math.min(max, maxEntropy) : null;

    if (validMin === null && validMax === null) {
      onFiltersChange({ ...filters, entropyRange: null });
    } else {
      onFiltersChange({
        ...filters,
        entropyRange: [validMin ?? 0, validMax ?? maxEntropy],
      });
    }
  };

  const handleMotifToggle = (motif: MotifType) => {
    const newMotifs = filters.motifTypes.includes(motif)
      ? filters.motifTypes.filter((m) => m !== motif)
      : [...filters.motifTypes, motif];
    onFiltersChange({ ...filters, motifTypes: newMotifs });
  };

  const handleSavePreset = () => {
    if (presetName.trim()) {
      onSavePreset(presetName.trim());
      setPresetName('');
      setShowPresetDialog(false);
    }
  };

  return (
    <div className="space-y-6 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="font-semibold">
          Filters
          {activeFilterCount > 0 && (
            <span className="ml-2 rounded-full bg-primary px-2 py-0.5 text-xs text-primary-foreground">
              {activeFilterCount}
            </span>
          )}
        </h3>
        <Button variant="ghost" size="sm" onClick={handleReset} className="gap-1">
          <RotateCcw className="h-3 w-3" />
          Reset
        </Button>
      </div>

      {/* Sequence Search */}
      <div className="space-y-2">
        <label className="text-sm font-medium">Sequence Search</label>
        <div className="relative">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search in k-mer sequences..."
            value={filters.sequenceQuery}
            onChange={(e) => onFiltersChange({ ...filters, sequenceQuery: e.target.value })}
            className="w-full rounded-md border bg-background py-2 pl-9 pr-8"
          />
          {filters.sequenceQuery && (
            <button
              onClick={() => onFiltersChange({ ...filters, sequenceQuery: '' })}
              className="absolute right-2 top-1/2 -translate-y-1/2"
              aria-label="Clear search"
            >
              <X className="h-4 w-4 text-muted-foreground" />
            </button>
          )}
        </div>
      </div>

      {/* Position Range */}
      <div className="space-y-2">
        <label className="text-sm font-medium">Position Range</label>
        <div className="flex items-center gap-2">
          <input
            type="number"
            placeholder="From"
            min={minPos}
            max={maxPos}
            value={filters.positionRange?.[0] ?? ''}
            onChange={(e) => {
              const parsed = e.target.value.trim() ? parseFloat(e.target.value) : NaN;
              handlePositionRangeChange(
                Number.isFinite(parsed) ? parsed : null,
                filters.positionRange?.[1] ?? null
              );
            }}
            className="w-full rounded-md border bg-background px-3 py-2"
          />
          <span className="text-muted-foreground">to</span>
          <input
            type="number"
            placeholder="To"
            min={minPos}
            max={maxPos}
            value={filters.positionRange?.[1] ?? ''}
            onChange={(e) => {
              const parsed = e.target.value.trim() ? parseFloat(e.target.value) : NaN;
              handlePositionRangeChange(
                filters.positionRange?.[0] ?? null,
                Number.isFinite(parsed) ? parsed : null
              );
            }}
            className="w-full rounded-md border bg-background px-3 py-2"
          />
        </div>
      </div>

      {/* Entropy Range */}
      <div className="space-y-2">
        <label className="text-sm font-medium">Entropy Range</label>
        <div className="flex items-center gap-2">
          <input
            type="number"
            placeholder="Min"
            step={0.1}
            min={0}
            value={filters.entropyRange?.[0] ?? ''}
            onChange={(e) => {
              const parsed = e.target.value.trim() ? parseFloat(e.target.value) : NaN;
              handleEntropyRangeChange(
                Number.isFinite(parsed) ? parsed : null,
                filters.entropyRange?.[1] ?? null
              );
            }}
            className="w-full rounded-md border bg-background px-3 py-2"
          />
          <span className="text-muted-foreground">to</span>
          <input
            type="number"
            placeholder="Max"
            step={0.1}
            value={filters.entropyRange?.[1] ?? ''}
            onChange={(e) => {
              const parsed = e.target.value.trim() ? parseFloat(e.target.value) : NaN;
              handleEntropyRangeChange(
                filters.entropyRange?.[0] ?? null,
                Number.isFinite(parsed) ? parsed : null
              );
            }}
            className="w-full rounded-md border bg-background px-3 py-2"
          />
        </div>
      </div>

      {/* Motif Types */}
      <div className="space-y-2">
        <label className="text-sm font-medium">Motif Types</label>
        <div className="flex flex-wrap gap-2">
          {MOTIF_OPTIONS.map((option) => (
            <button
              key={option.value}
              onClick={() => handleMotifToggle(option.value)}
              aria-pressed={filters.motifTypes.includes(option.value)}
              aria-label={`Filter by ${option.label} motif`}
              className={`rounded-full px-3 py-1 text-sm font-medium transition-colors ${
                filters.motifTypes.includes(option.value)
                  ? 'text-white'
                  : 'bg-muted text-muted-foreground'
              }`}
              style={{
                backgroundColor: filters.motifTypes.includes(option.value)
                  ? option.color
                  : undefined,
              }}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      {/* Low Support Toggle */}
      <label className="flex items-center gap-3">
        <input
          type="checkbox"
          checked={filters.includeLowSupport}
          onChange={(e) => onFiltersChange({ ...filters, includeLowSupport: e.target.checked })}
          className="h-4 w-4 rounded border-input"
        />
        <span className="text-sm">Include low support positions</span>
      </label>

      {/* Presets */}
      <div className="space-y-2 border-t pt-4">
        <div className="flex items-center justify-between">
          <label className="text-sm font-medium">Filter Presets</label>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setShowPresetDialog(true)}
            className="gap-1"
          >
            <Save className="h-3 w-3" />
            Save
          </Button>
        </div>

        {presets.length > 0 ? (
          <div className="space-y-1">
            {presets.map((preset) => (
              <div
                key={preset.id}
                className="flex items-center justify-between rounded-md p-2 hover:bg-muted"
              >
                <button
                  onClick={() => onLoadPreset(preset)}
                  className="text-sm"
                >
                  {preset.name}
                </button>
                <button
                  onClick={() => onDeletePreset(preset.id)}
                  className="text-muted-foreground hover:text-destructive"
                  aria-label={`Delete preset ${preset.name}`}
                >
                  <X className="h-3 w-3" />
                </button>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">No saved presets</p>
        )}

        {/* Save Preset Dialog */}
        {showPresetDialog && (
          <div className="flex items-center gap-2">
            <input
              type="text"
              placeholder="Preset name..."
              value={presetName}
              onChange={(e) => setPresetName(e.target.value)}
              className="flex-1 rounded-md border bg-background px-3 py-1 text-sm"
              autoFocus
            />
            <Button size="sm" onClick={handleSavePreset}>
              Save
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowPresetDialog(false)}
            >
              Cancel
            </Button>
          </div>
        )}
      </div>
    </div>
  );
});
