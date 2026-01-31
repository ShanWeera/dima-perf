/**
 * DiMA Desktop - TypeScript Type Definitions
 * 
 * Core types that mirror the Rust backend data structures.
 */

// ============================================================================
// Analysis Types
// ============================================================================

/**
 * Complete analysis results from DiMA
 */
export interface AnalysisResult {
  sequence_count: number;
  support_threshold: number;
  low_support_count: number;
  query_name: string;
  kmer_length: number;
  highest_entropy: HighestEntropy;
  average_entropy: number;
  results: Position[];
}

/**
 * Highest entropy position info
 */
export interface HighestEntropy {
  position: number;
  entropy: number;
}

/**
 * Position-level analysis data
 */
export interface Position {
  position: number;
  low_support: string | null;
  entropy: number;
  support: number;
  distinct_variants_count: number;
  distinct_variants_incidence: number;
  total_variants_incidence: number;
  diversity_motifs: Variant[] | null;
}

/**
 * Variant (k-mer) data
 */
export interface Variant {
  sequence: string;
  count: number;
  incidence: number;
  motif_short: string | null;
  motif_long: string | null;
  metadata: Record<string, Record<string, number>> | null;
}

/**
 * Motif type classification
 */
export type MotifType = 'I' | 'Ma' | 'Mi' | 'U';

// ============================================================================
// Project Types
// ============================================================================

/**
 * Project metadata
 */
export interface Project {
  name: string;
  path: string;
  createdAt: string;
  hasResults: boolean;
  hasInputFile: boolean;
  inputFileName?: string;
}

/**
 * Recent project entry
 */
export interface RecentProject {
  name: string;
  path: string;
  last_opened: string;
  input_file_name?: string;
  sequence_count?: number;
}

// ============================================================================
// Configuration Types
// ============================================================================

/**
 * Analysis configuration
 */
export interface AnalysisConfig {
  kmerLength: number;
  supportThreshold: number;
  queryName: string;
  alphabet: 'protein' | 'nucleotide';
  headerFormat: string | null;
  metadataFields: string | null;
  validationMode: 'strict' | 'permissive' | 'report';
  allowLowercase: boolean;
  hcsEnabled: boolean;
  hcsThreshold: number | null;
}

/**
 * Default analysis configuration
 */
export const DEFAULT_ANALYSIS_CONFIG: AnalysisConfig = {
  kmerLength: 9,
  supportThreshold: 30,
  queryName: '',
  alphabet: 'protein',
  headerFormat: null,
  metadataFields: null,
  validationMode: 'strict',
  allowLowercase: false,
  hcsEnabled: false,
  hcsThreshold: null,
};

// ============================================================================
// Validation Types
// ============================================================================

/**
 * FASTA file validation result
 */
export interface FastaValidation {
  is_valid: boolean;
  sequence_count: number;
  sequence_length: number | null;
  sample_headers: string[];
  detected_alphabet: string;
  errors: ValidationError[];
  file_size_bytes: number;
  file_modified_at: string | null;
}

/**
 * Validation error
 */
export interface ValidationError {
  error_type: string;
  message: string;
  line_number: number | null;
}

/**
 * Header format detection result
 */
export interface HeaderFormatDetection {
  detected_format: string | null;
  detected_delimiter: string | null;
  field_count: number;
  sample_parsed: ParsedHeader[];
  suggested_fields: string[];
}

/**
 * Parsed header example
 */
export interface ParsedHeader {
  raw: string;
  fields: string[];
}

// ============================================================================
// Progress Types
// ============================================================================

/**
 * Analysis progress update
 */
export interface ProgressUpdate {
  stage: AnalysisStage;
  current: number;
  total: number;
  message: string;
  throughput?: number;
}

/**
 * Analysis stages
 */
export type AnalysisStage =
  | 'reading_fasta'
  | 'kmer_extraction'
  | 'entropy_calculation'
  | 'output_generation'
  | 'complete';

// ============================================================================
// Annotation Types
// ============================================================================

/**
 * Position annotation
 */
export interface Annotation {
  id: string;
  positionNumber: number;
  color: AnnotationColor;
  label: string;
  note: string;
  createdAt: string;
}

/**
 * Available annotation colors
 */
export type AnnotationColor =
  | 'red'
  | 'orange'
  | 'amber'
  | 'yellow'
  | 'lime'
  | 'green'
  | 'teal'
  | 'cyan'
  | 'blue'
  | 'indigo'
  | 'purple'
  | 'pink';

/**
 * Annotation color values for CSS
 */
export const ANNOTATION_COLORS: Record<AnnotationColor, string> = {
  red: '#ef4444',
  orange: '#f97316',
  amber: '#f59e0b',
  yellow: '#eab308',
  lime: '#84cc16',
  green: '#22c55e',
  teal: '#14b8a6',
  cyan: '#06b6d4',
  blue: '#3b82f6',
  indigo: '#6366f1',
  purple: '#a855f7',
  pink: '#ec4899',
};

// ============================================================================
// Filter Types
// ============================================================================

/**
 * Search/filter options
 */
export interface SearchFilters {
  positionRange: [number, number] | null;
  sequenceQuery: string;
  entropyRange: [number, number] | null;
  motifTypes: MotifType[];
  includeLowSupport: boolean;
}

/**
 * Default search filters
 */
export const DEFAULT_SEARCH_FILTERS: SearchFilters = {
  positionRange: null,
  sequenceQuery: '',
  entropyRange: null,
  motifTypes: ['I', 'Ma', 'Mi', 'U'],
  includeLowSupport: true,
};

/**
 * Filter preset
 */
export interface FilterPreset {
  id: string;
  name: string;
  filters: SearchFilters;
}

// ============================================================================
// Settings Types
// ============================================================================

/**
 * Application settings
 */
export interface AppSettings {
  theme: 'light' | 'dark' | 'system';
  decimalPrecision: number;
  defaultOutputDirectory: string | null;
  defaultChartDpi: 72 | 300;
  defaultKmerLength: number;
  defaultSupportThreshold: number;
  defaultValidationMode: 'strict' | 'permissive' | 'report';
  lastUsedConfig: Partial<AnalysisConfig> | null;
}

/**
 * Default application settings
 */
export const DEFAULT_APP_SETTINGS: AppSettings = {
  theme: 'system',
  decimalPrecision: 4,
  defaultOutputDirectory: null,
  defaultChartDpi: 72,
  defaultKmerLength: 9,
  defaultSupportThreshold: 30,
  defaultValidationMode: 'strict',
  lastUsedConfig: null,
};
