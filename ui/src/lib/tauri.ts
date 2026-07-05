/**
 * DiMA Desktop - Tauri API Wrappers
 * 
 * Type-safe wrappers around Tauri invoke commands.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * Invoke a Tauri command with a timeout guard.
 * If the command doesn't complete within `timeoutMs`, the returned promise rejects
 * with a descriptive timeout error. This prevents the UI from hanging indefinitely
 * when the backend is stuck (e.g., I/O deadlock, infinite loop, network stall).
 */
function invokeWithTimeout<T>(
  cmd: string,
  args?: Record<string, unknown>,
  timeoutMs = 300_000, // 5 minutes default
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      reject(new Error(`Command '${cmd}' timed out after ${Math.round(timeoutMs / 1000)}s`));
    }, timeoutMs);

    invoke<T>(cmd, args)
      .then((result) => {
        if (!timedOut) {
          clearTimeout(timer);
          resolve(result);
        }
      })
      .catch((err) => {
        if (!timedOut) {
          clearTimeout(timer);
          reject(err);
        }
      });
  });
}
import type {
  AnalysisResult,
  AnnotationColor,
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
  config?: Record<string, unknown>;
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
 * Load analysis results from a project.
 * Backend validates the file and returns parsed JSON.
 * Performs runtime shape validation to fail fast with clear errors rather than
 * deep crashes inside chart components from corrupt/partial data. (Fix S6)
 */
export async function loadResults(projectPath: string): Promise<AnalysisResult> {
  const raw = await invokeWithTimeout<AnalysisResult>('load_results', { projectPath }, 120_000);
  validateResultsShape(raw);
  return raw;
}

/**
 * Lightweight runtime validation for results shape.
 * Catches corrupt/wrong-schema files at load time instead of deep in chart rendering.
 */
function validateResultsShape(data: unknown): asserts data is AnalysisResult {
  if (!data || typeof data !== 'object') {
    throw new Error('Results data is not an object');
  }
  const obj = data as Record<string, unknown>;
  if (typeof obj.sequence_count !== 'number' || typeof obj.kmer_length !== 'number') {
    throw new Error('Results missing required numeric fields (sequence_count, kmer_length)');
  }
  if (!Array.isArray(obj.results)) {
    throw new Error('Results data missing "results" array');
  }
  if (obj.results.length > 0) {
    const first = obj.results[0] as Record<string, unknown>;
    if (typeof first.position !== 'number' || typeof first.entropy !== 'number') {
      throw new Error('Results positions have invalid shape (missing position or entropy)');
    }
    if (!Array.isArray(first.diversity_motifs) && first.diversity_motifs !== null) {
      throw new Error('Results positions missing "diversity_motifs" field');
    }
  }
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
 * Atomically drain file paths queued during cold-start (before the event listener mounted).
 * Called once on frontend mount to handle .dima file-association launches. (Fix 4.42)
 */
export async function takePendingOpenPaths(): Promise<string[]> {
  return invoke('take_pending_open_paths');
}

/**
 * Get project path by name
 */
// ============================================================================
// Validation Commands
// ============================================================================

/**
 * Validate a FASTA file (may scan large files — 2 min timeout)
 */
export async function validateFasta(
  path: string,
  alphabet?: string
): Promise<FastaValidation> {
  return invokeWithTimeout('validate_fasta', { path, alphabet }, 120_000);
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
  /** File fingerprint from validation for TOCTOU detection (Fix 4.30) */
  validated_file_size: number | null;
  validated_file_modified_at: string | null;
}

export interface AnalysisResponse {
  success: boolean;
  results_path?: string;
  sequence_count: number;
  position_count: number;
  average_entropy: number;
  highest_entropy_position: number;
  highest_entropy_value: number;
  /** Non-fatal warnings about the analysis (e.g., threshold > sequence count) */
  warnings?: string[];
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
 * Cancel the current validation task (if running). (Fix 4.29)
 * Prevents wasted CPU/IO when user selects a different file or navigates away.
 */
export async function cancelValidation(): Promise<void> {
  return invoke('cancel_validation');
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

/**
 * Listen for analysis warnings (e.g. config save failure after successful analysis).
 * These are non-fatal warnings that the user should see. (Fix 5.11)
 */
export async function onAnalysisWarning(
  callback: (message: string) => void
): Promise<UnlistenFn> {
  return listen<string>('analysis-warning', (event) => {
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
 * Get the full projects directory path, constructed platform-correctly
 * by the Rust backend using std::path::Path::join. (Fix 4.49)
 */
export async function getProjectsDirectoryPath(): Promise<string> {
  return invoke('get_projects_directory_path');
}

/**
 * Reveal a path in the system file explorer
 */
export async function revealInExplorer(path: string): Promise<void> {
  return invoke('reveal_in_explorer', { path });
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

const VALID_ANNOTATION_COLORS: ReadonlySet<string> = new Set<AnnotationColor>([
  'red', 'orange', 'amber', 'yellow', 'lime', 'green',
  'teal', 'cyan', 'blue', 'indigo', 'purple', 'pink',
]);

const DEFAULT_ANNOTATION_COLOR: AnnotationColor = 'blue';

/**
 * Load annotations from project.
 * Validates color values at the IPC boundary to prevent type unsoundness.
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
  
  return backendAnnotations.map(a => ({
    id: a.id,
    positionNumber: a.position_number,
    color: VALID_ANNOTATION_COLORS.has(a.color)
      ? (a.color as AnnotationColor)
      : DEFAULT_ANNOTATION_COLOR,
    label: a.label,
    note: a.note,
    createdAt: a.created_at,
  }));
}

// ============================================================================
// Filter Persistence Commands
// ============================================================================

import type { MotifType, SearchFilters, FilterPreset } from './types';

const VALID_MOTIF_TYPES: ReadonlySet<string> = new Set<MotifType>(['I', 'Ma', 'Mi', 'U']);

/** Filter and validate motif type strings from the backend IPC boundary */
function validateMotifTypes(raw: string[]): MotifType[] {
  return raw.filter((t): t is MotifType => VALID_MOTIF_TYPES.has(t));
}

/**
 * Save filters to project
 */
export async function saveFilters(projectPath: string, filters: SearchFilters): Promise<void> {
  // Map ±Infinity sentinels back to null for JSON/IPC safety. (Fix 5.108)
  // JSON.stringify(Infinity) produces null; serde_json may reject non-finite floats.
  const toFiniteOrNull = (val: number | null | undefined): number | null =>
    val !== undefined && val !== null && Number.isFinite(val) ? val : null;

  const backendFilters = {
    position_from: toFiniteOrNull(filters.positionRange?.[0]),
    position_to: toFiniteOrNull(filters.positionRange?.[1]),
    sequence_query: filters.sequenceQuery,
    entropy_min: toFiniteOrNull(filters.entropyRange?.[0]),
    entropy_max: toFiniteOrNull(filters.entropyRange?.[1]),
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
  
  // Convert from snake_case to camelCase.
  // Restore partial ranges: if only one bound was set (min without max or vice versa),
  // still reconstruct the tuple so the filter is not silently dropped on reload.
  const hasPositionRange = backendFilters.position_from !== null || backendFilters.position_to !== null;
  const hasEntropyRange = backendFilters.entropy_min !== null || backendFilters.entropy_max !== null;

  // Use -Infinity/Infinity as sentinel for "unset" bounds so the filter
  // still applies correctly (any position >= -Infinity passes). (Fix 4.10)
  return {
    positionRange: hasPositionRange
      ? [backendFilters.position_from ?? -Infinity, backendFilters.position_to ?? Infinity]
      : null,
    sequenceQuery: backendFilters.sequence_query,
    entropyRange: hasEntropyRange
      ? [backendFilters.entropy_min ?? -Infinity, backendFilters.entropy_max ?? Infinity]
      : null,
    motifTypes: validateMotifTypes(backendFilters.motif_types),
    includeLowSupport: backendFilters.include_low_support,
  };
}

/**
 * Save global filter presets
 */
export async function saveFilterPresets(presets: FilterPreset[]): Promise<void> {
  const toFiniteOrNull = (val: number | null | undefined): number | null =>
    val !== undefined && val !== null && Number.isFinite(val) ? val : null;

  const backendPresets = presets.map(p => ({
    id: p.id,
    name: p.name,
    filters: {
      position_from: toFiniteOrNull(p.filters.positionRange?.[0]),
      position_to: toFiniteOrNull(p.filters.positionRange?.[1]),
      sequence_query: p.filters.sequenceQuery,
      entropy_min: toFiniteOrNull(p.filters.entropyRange?.[0]),
      entropy_max: toFiniteOrNull(p.filters.entropyRange?.[1]),
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
  return backendPresets.map(p => {
    const hasPositionRange = p.filters.position_from !== null || p.filters.position_to !== null;
    const hasEntropyRange = p.filters.entropy_min !== null || p.filters.entropy_max !== null;
    return {
      id: p.id,
      name: p.name,
      filters: {
        positionRange: hasPositionRange
          ? [p.filters.position_from ?? -Infinity, p.filters.position_to ?? Infinity]
          : null,
        sequenceQuery: p.filters.sequence_query,
        entropyRange: hasEntropyRange
          ? [p.filters.entropy_min ?? -Infinity, p.filters.entropy_max ?? Infinity]
          : null,
        motifTypes: validateMotifTypes(p.filters.motif_types),
        includeLowSupport: p.filters.include_low_support,
      },
    };
  });
}

// ============================================================================
// PDB Structure Commands
// ============================================================================

import type { ChainInfo, PositionMapping, UniProtInfo } from './types';

/**
 * Fetch a PDB file from RCSB PDB by ID (network — 30s timeout)
 */
export async function fetchPdb(pdbId: string): Promise<string> {
  return invokeWithTimeout('fetch_pdb', { pdbId }, 30_000);
}

/**
 * Parse PDB content and extract sequence information for each chain (60s timeout)
 */
export async function parsePdbSequence(pdbContent: string): Promise<ChainInfo[]> {
  return invokeWithTimeout('parse_pdb_sequence', { pdbContent }, 60_000);
}

/**
 * Align MSA sequence to PDB sequence and return position mapping
 */
export async function alignSequences(
  msaSequence: string,
  pdbSequence: string,
  pdbResidueNumbers: number[]
): Promise<PositionMapping> {
  return invokeWithTimeout('align_sequences', { msaSequence, pdbSequence, pdbResidueNumbers }, 60_000);
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

// ============================================================================
// UniProt Commands
// ============================================================================

/**
 * Look up the UniProt accession associated with a PDB polymer entity (network — 30s timeout)
 */
export async function fetchUniProtAccession(
  pdbId: string,
  entityId: number
): Promise<string> {
  return invokeWithTimeout('fetch_uniprot_accession', { pdbId, entityId }, 30_000);
}

/**
 * Fetch protein information and feature annotations from UniProt (network — 30s timeout)
 */
export async function fetchUniProtFeatures(
  accession: string
): Promise<UniProtInfo> {
  return invokeWithTimeout('fetch_uniprot_features', { accession }, 30_000);
}
