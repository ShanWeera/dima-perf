/**
 * DiMA Desktop - Tauri API Wrappers
 * 
 * Type-safe wrappers around Tauri invoke commands.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  FastaValidation,
  HeaderFormatDetection,
  ProgressUpdate,
  RecentProject,
  AppSettings,
} from './types';

// ============================================================================
// Project Commands
// ============================================================================

export interface CreateProjectResponse {
  path: string;
  name: string;
}

export interface ProjectInfo {
  name: string;
  path: string;
  created_at: string;
  has_results: boolean;
  has_input_file: boolean;
  input_file_name?: string;
}

/**
 * Create a new project
 */
export async function createProject(name: string): Promise<CreateProjectResponse> {
  return invoke('create_project', { name });
}

/**
 * Open an existing project
 */
export async function openProject(path: string): Promise<ProjectInfo> {
  return invoke('open_project', { path });
}

/**
 * List recent projects
 */
export async function listRecentProjects(): Promise<RecentProject[]> {
  return invoke('list_recent_projects');
}

/**
 * Clear all recent projects from the list
 */
export async function clearRecentProjects(): Promise<void> {
  return invoke('clear_recent_projects');
}

/**
 * Delete a project
 */
export async function deleteProject(path: string): Promise<void> {
  return invoke('delete_project', { path });
}

/**
 * Get project path by name
 */
export async function getProjectPath(name: string): Promise<string> {
  return invoke('get_project_path', { name });
}

// ============================================================================
// Validation Commands
// ============================================================================

/**
 * Validate a FASTA file
 */
export async function validateFasta(
  path: string,
  alphabet?: string
): Promise<FastaValidation> {
  return invoke('validate_fasta', { path, alphabet });
}

/**
 * Detect header format from a FASTA file
 */
export async function detectHeaderFormat(
  path: string
): Promise<HeaderFormatDetection> {
  return invoke('detect_header_format', { path });
}

// ============================================================================
// Analysis Commands
// ============================================================================

export interface AnalysisRequest {
  project_path: string;
  input_path: string;
  copy_input: boolean;
  kmer_length: number;
  support_threshold: number;
  query_name: string;
  alphabet: string;
  header_format: string | null;
  metadata_fields: string | null;
  validation_mode: string;
  allow_lowercase: boolean;
  hcs_enabled: boolean;
  hcs_threshold: number | null;
}

export interface AnalysisResponse {
  success: boolean;
  results_path?: string;
  sequence_count: number;
  position_count: number;
  average_entropy: number;
  highest_entropy_position: number;
  highest_entropy_value: number;
}

/**
 * Run DiMA analysis
 */
export async function runAnalysis(
  request: AnalysisRequest
): Promise<AnalysisResponse> {
  return invoke('run_analysis', { request });
}

/**
 * Cancel the current analysis
 */
export async function cancelAnalysis(): Promise<void> {
  return invoke('cancel_analysis');
}

/**
 * Listen for analysis progress updates
 */
export async function onAnalysisProgress(
  callback: (progress: ProgressUpdate) => void
): Promise<UnlistenFn> {
  return listen<ProgressUpdate>('analysis-progress', (event) => {
    callback(event.payload);
  });
}

// ============================================================================
// Export Commands
// ============================================================================

export interface ExportRequest {
  project_path: string;
  output_path: string;
  format: 'json' | 'dima';
  compression?: number;
}

export interface ExportResponse {
  success: boolean;
  output_path: string;
  file_size: number;
}

/**
 * Export results to a file
 */
export async function exportResults(
  request: ExportRequest
): Promise<ExportResponse> {
  return invoke('export_results', { request });
}

export interface ChartExportRequest {
  data_url: string;
  output_path: string;
  format: string;
  title?: string;
}

/**
 * Export a chart image
 */
export async function exportChart(
  request: ChartExportRequest
): Promise<ExportResponse> {
  return invoke('export_chart', { request });
}

export interface ImportDimaRequest {
  file_path: string;
  project_path: string;
}

/**
 * Import a .dima binary file into a project
 */
export async function importDimaFile(
  request: ImportDimaRequest
): Promise<ExportResponse> {
  return invoke('import_dima_file', { request });
}

// ============================================================================
// Settings Commands
// ============================================================================

/**
 * Get application settings
 */
export async function getSettings(): Promise<AppSettings> {
  return invoke('get_settings');
}

/**
 * Update application settings
 */
export async function updateSettings(settings: AppSettings): Promise<void> {
  return invoke('update_settings', { settings });
}

/**
 * Get the Documents folder path
 */
export async function getDocumentsPath(): Promise<string> {
  return invoke('get_documents_path');
}

/**
 * Reveal a path in the system file explorer
 */
export async function revealInExplorer(path: string): Promise<void> {
  return invoke('reveal_in_explorer', { path });
}

/**
 * Create a new application window
 */
export async function createNewWindow(): Promise<void> {
  return invoke('create_new_window');
}

// ============================================================================
// Layout Persistence Commands
// ============================================================================

export interface LayoutItem {
  i: string;
  x: number;
  y: number;
  w: number;
  h: number;
  minW?: number;
  minH?: number;
}

export interface DashboardLayout {
  layout: LayoutItem[];
  hidden_panels: string[];
}

/**
 * Save dashboard layout to project
 */
export async function saveLayout(projectPath: string, layout: DashboardLayout): Promise<void> {
  return invoke('save_layout', { projectPath, layout });
}

/**
 * Load dashboard layout from project
 */
export async function loadLayout(projectPath: string): Promise<DashboardLayout | null> {
  return invoke('load_layout', { projectPath });
}

// ============================================================================
// Annotation Persistence Commands
// ============================================================================

import type { Annotation } from './types';

/**
 * Save annotations to project
 */
export async function saveAnnotations(projectPath: string, annotations: Annotation[]): Promise<void> {
  // Convert to snake_case for Rust backend
  const backendAnnotations = annotations.map(a => ({
    id: a.id,
    position_number: a.positionNumber,
    color: a.color,
    label: a.label,
    note: a.note,
    created_at: a.createdAt,
  }));
  return invoke('save_annotations', { projectPath, annotations: backendAnnotations });
}

/**
 * Load annotations from project
 */
export async function loadAnnotations(projectPath: string): Promise<Annotation[]> {
  const backendAnnotations: Array<{
    id: string;
    position_number: number;
    color: string;
    label: string;
    note: string;
    created_at: string;
  }> = await invoke('load_annotations', { projectPath });
  
  // Convert from snake_case to camelCase
  return backendAnnotations.map(a => ({
    id: a.id,
    positionNumber: a.position_number,
    color: a.color as Annotation['color'],
    label: a.label,
    note: a.note,
    createdAt: a.created_at,
  }));
}

// ============================================================================
// Filter Persistence Commands
// ============================================================================

import type { SearchFilters, FilterPreset } from './types';

/**
 * Save filters to project
 */
export async function saveFilters(projectPath: string, filters: SearchFilters): Promise<void> {
  // Convert to snake_case for Rust backend
  const backendFilters = {
    position_from: filters.positionRange?.[0] ?? null,
    position_to: filters.positionRange?.[1] ?? null,
    sequence_query: filters.sequenceQuery,
    entropy_min: filters.entropyRange?.[0] ?? null,
    entropy_max: filters.entropyRange?.[1] ?? null,
    motif_types: filters.motifTypes,
    include_low_support: filters.includeLowSupport,
  };
  return invoke('save_filters', { projectPath, filters: backendFilters });
}

/**
 * Load filters from project
 */
export async function loadFilters(projectPath: string): Promise<SearchFilters | null> {
  const backendFilters: {
    position_from: number | null;
    position_to: number | null;
    sequence_query: string;
    entropy_min: number | null;
    entropy_max: number | null;
    motif_types: string[];
    include_low_support: boolean;
  } | null = await invoke('load_filters', { projectPath });
  
  if (!backendFilters) return null;
  
  // Convert from snake_case to camelCase
  return {
    positionRange: backendFilters.position_from !== null && backendFilters.position_to !== null 
      ? [backendFilters.position_from, backendFilters.position_to] 
      : null,
    sequenceQuery: backendFilters.sequence_query,
    entropyRange: backendFilters.entropy_min !== null && backendFilters.entropy_max !== null
      ? [backendFilters.entropy_min, backendFilters.entropy_max]
      : null,
    motifTypes: backendFilters.motif_types as SearchFilters['motifTypes'],
    includeLowSupport: backendFilters.include_low_support,
  };
}

/**
 * Save global filter presets
 */
export async function saveFilterPresets(presets: FilterPreset[]): Promise<void> {
  // Convert to snake_case for Rust backend
  const backendPresets = presets.map(p => ({
    id: p.id,
    name: p.name,
    filters: {
      position_from: p.filters.positionRange?.[0] ?? null,
      position_to: p.filters.positionRange?.[1] ?? null,
      sequence_query: p.filters.sequenceQuery,
      entropy_min: p.filters.entropyRange?.[0] ?? null,
      entropy_max: p.filters.entropyRange?.[1] ?? null,
      motif_types: p.filters.motifTypes,
      include_low_support: p.filters.includeLowSupport,
    },
  }));
  return invoke('save_filter_presets', { presets: backendPresets });
}

/**
 * Load global filter presets
 */
export async function loadFilterPresets(): Promise<FilterPreset[]> {
  const backendPresets: Array<{
    id: string;
    name: string;
    filters: {
      position_from: number | null;
      position_to: number | null;
      sequence_query: string;
      entropy_min: number | null;
      entropy_max: number | null;
      motif_types: string[];
      include_low_support: boolean;
    };
  }> = await invoke('load_filter_presets');
  
  // Convert from snake_case to camelCase
  return backendPresets.map(p => ({
    id: p.id,
    name: p.name,
    filters: {
      positionRange: p.filters.position_from !== null && p.filters.position_to !== null
        ? [p.filters.position_from, p.filters.position_to]
        : null,
      sequenceQuery: p.filters.sequence_query,
      entropyRange: p.filters.entropy_min !== null && p.filters.entropy_max !== null
        ? [p.filters.entropy_min, p.filters.entropy_max]
        : null,
      motifTypes: p.filters.motif_types as SearchFilters['motifTypes'],
      includeLowSupport: p.filters.include_low_support,
    },
  }));
}

// ============================================================================
// PDB Structure Commands
// ============================================================================

import type { ChainInfo, PositionMapping } from './types';

/**
 * Fetch a PDB file from RCSB PDB by ID
 */
export async function fetchPdb(pdbId: string): Promise<string> {
  return invoke('fetch_pdb', { pdbId });
}

/**
 * Parse PDB content and extract sequence information for each chain
 */
export async function parsePdbSequence(pdbContent: string): Promise<ChainInfo[]> {
  return invoke('parse_pdb_sequence', { pdbContent });
}

/**
 * Align MSA sequence to PDB sequence and return position mapping
 */
export async function alignSequences(
  msaSequence: string,
  pdbSequence: string,
  pdbResidueNumbers: number[]
): Promise<PositionMapping> {
  return invoke('align_sequences', { msaSequence, pdbSequence, pdbResidueNumbers });
}

/**
 * Create a direct 1:1 position mapping with an optional offset
 */
export async function createDirectMapping(
  msaPositions: number[],
  pdbResidueNumbers: number[],
  offset: number
): Promise<PositionMapping> {
  return invoke('create_direct_mapping', { msaPositions, pdbResidueNumbers, offset });
}
