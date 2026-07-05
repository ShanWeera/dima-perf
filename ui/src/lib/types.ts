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
export type LowSupportTag = 'NS' | 'LS' | 'ELS';

export interface Position {
  position: number;
  low_support: LowSupportTag | null;
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
  motif_short: MotifType | null;
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
  supportThreshold: 100,
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
  detected_alphabet: 'protein' | 'nucleotide' | 'unknown';
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

// Default search filters are defined in @/lib/filters.ts (single source of truth)

/**
 * Filter preset
 */
export interface FilterPreset {
  id: string;
  name: string;
  filters: SearchFilters;
}

// ============================================================================
// PDB/Structure Types
// ============================================================================

/**
 * Information about a chain in a PDB file
 */
export interface ChainInfo {
  chain_id: string;
  sequence: string;
  residue_numbers: number[];
}

/**
 * Position mapping between MSA positions and PDB residue numbers
 */
export interface PositionMapping {
  msa_to_pdb: Record<number, number>;
  alignment_score: number;
  coverage: number;
}


// ============================================================================
// UniProt / Protein Feature Types
// ============================================================================

/**
 * A single protein feature annotation from UniProt
 */
export interface ProteinFeature {
  feature_type: string;
  category: string;
  description: string;
  /** Start position in UniProt numbering (1-based) */
  begin: number;
  /** End position in UniProt numbering (1-based) */
  end: number;
  evidences: string[];
}

/**
 * Resolved UniProt protein metadata together with its features
 */
export interface UniProtInfo {
  accession: string;
  protein_name: string;
  organism: string;
  sequence_length: number;
  /** Full UniProt canonical sequence (for alignment to PDB) */
  sequence: string;
  features: ProteinFeature[];
}

/**
 * Configuration for a feature category (color, label, SVG shape)
 */
export interface FeatureCategoryConfig {
  color: string;
  label: string;
  /** rect for range features, circle for point features */
  shape: 'rect' | 'circle';
  /** Which UniProt feature_type values belong to this category */
  uniprotTypes: string[];
}

/**
 * A feature with positions mapped to the MSA coordinate space
 */
export interface MappedFeature extends ProteinFeature {
  /** Start position in MSA coordinates (null if unmappable) */
  msaBegin: number | null;
  /** End position in MSA coordinates (null if unmappable) */
  msaEnd: number | null;
  /** Category key from FEATURE_CATEGORIES (e.g. "DOMAIN") */
  categoryKey: string;
}

// ============================================================================
// Settings Types
// ============================================================================

/**
 * Application settings
 */
export interface AppSettings {
  schemaVersion: number;
  theme: 'light' | 'dark' | 'system';
  decimalPrecision: number;
  defaultOutputDirectory: string | null;
  /** Rust backend clamps to 36-600 */
  defaultChartDpi: number;
  defaultKmerLength: number;
  defaultSupportThreshold: number;
  defaultValidationMode: 'strict' | 'permissive' | 'report';
  lastUsedConfig: Partial<AnalysisConfig> | null;
}

/**
 * Default application settings
 */
export const DEFAULT_APP_SETTINGS: AppSettings = {
  schemaVersion: 1,
  theme: 'system',
  decimalPrecision: 4,
  defaultOutputDirectory: null,
  defaultChartDpi: 72,
  defaultKmerLength: 9,
  defaultSupportThreshold: 100,
  defaultValidationMode: 'strict',
  lastUsedConfig: null,
};
