/**
 * DiMA Desktop - Project Store
 * 
 * Manages the current project state and analysis data.
 * Uses generation counters to prevent stale async results from overwriting
 * newer state (e.g., when an analysis is cancelled, re-started, or project switched).
 */

import { create } from 'zustand';
import type { 
  Project, 
  AnalysisResult, 
  AnalysisConfig, 
  ProgressUpdate,
  Annotation,
  FastaValidation,
  HeaderFormatDetection,
} from '@/lib/types';
import { DEFAULT_ANALYSIS_CONFIG } from '@/lib/types';
import { 
  createProject, 
  openProject, 
  runAnalysis, 
  cancelAnalysis,
  cancelValidation,
  onAnalysisProgress,
  onAnalysisWarning,
  validateFasta,
  detectHeaderFormat,
  saveAnnotations,
  loadAnnotations,
  loadResults,
  updateSettings,
  getSettings,
} from '@/lib/tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore } from './appStore';
import { useToastStore } from './toastStore';
import { showErrorToast, extractErrorMessage } from '@/lib/utils';
import { useFeatureStore } from './featureStore';

// Monotonically increasing counter to detect stale async analysis operations.
let analysisGeneration = 0;

// Generation counter for validation operations — prevents stale validation
// results from overwriting state when files are changed rapidly.
let validationGeneration = 0;

// Generation counter for project open/create — prevents stale responses from
// a slow project open from overwriting a newer project's state.
let projectOpenGeneration = 0;

// Debounced annotation save to avoid excessive IPC on rapid edits.
// Re-reads from store state at execution time to prevent stale snapshot writes
// when a project switch happens between schedule and fire. (Fix 5.32)
let annotationSaveTimer: ReturnType<typeof setTimeout> | null = null;
let annotationSaveTargetPath: string | null = null;
const ANNOTATION_SAVE_DELAY_MS = 500;

function debouncedSaveAnnotations(projectPath: string, _annotations: Annotation[]) {
  if (annotationSaveTimer) clearTimeout(annotationSaveTimer);
  annotationSaveTargetPath = projectPath;
  annotationSaveTimer = setTimeout(() => {
    annotationSaveTimer = null;
    const state = useProjectStore.getState();
    // Only write if the current project still matches the target — prevents
    // writing stale annotations to the wrong project after a switch.
    if (state.currentProject?.path !== annotationSaveTargetPath) return;
    saveAnnotations(annotationSaveTargetPath, state.annotations).catch((error) => {
      showErrorToast('Failed to save annotations', error);
    });
    annotationSaveTargetPath = null;
  }, ANNOTATION_SAVE_DELAY_MS);
}

/**
 * Flush any pending debounced annotation save immediately.
 * Call this on app close to ensure no data is lost.
 * Always flushes even when annotations array is empty — saving `[]`
 * correctly clears disk state when user deleted all annotations.
 */
export function flushPendingAnnotationSave(): void {
  if (annotationSaveTimer) {
    clearTimeout(annotationSaveTimer);
    annotationSaveTimer = null;
    const state = useProjectStore.getState();
    const projectPath = state.currentProject?.path;
    if (projectPath) {
      saveAnnotations(projectPath, state.annotations).catch((error) => {
        showErrorToast('Failed to flush annotations on close', error);
      });
    }
  }
}

export type WizardStep = 'input' | 'configure' | 'analyzing' | 'results';

interface ProjectState {
  currentProject: Project | null;
  isLoadingProject: boolean;
  wizardStep: WizardStep;
  inputFilePath: string | null;
  copyInputFile: boolean;
  fastaValidation: FastaValidation | null;
  headerFormatDetection: HeaderFormatDetection | null;
  config: AnalysisConfig;
  isAnalyzing: boolean;
  progress: ProgressUpdate | null;
  analysisError: string | null;
  results: AnalysisResult | null;
  annotations: Annotation[];
  selectedPosition: number | null;
  // Incremented when layout is reset externally (e.g. from SettingsView) to
  // trigger ResultsStep to reload from disk. (Fix 5.91)
  layoutResetVersion: number;
  bumpLayoutResetVersion: () => void;

  createNewProject: (name: string) => Promise<void>;
  openExistingProject: (path: string) => Promise<void>;
  closeProject: () => void;
  setWizardStep: (step: WizardStep) => void;
  goBack: () => void;
  goNext: () => void;
  setInputFile: (path: string) => Promise<void>;
  setCopyInputFile: (copy: boolean) => void;
  updateConfig: (partial: Partial<AnalysisConfig>) => void;
  resetConfig: () => void;
  startAnalysis: () => Promise<void>;
  cancelCurrentAnalysis: () => Promise<void>;
  setResults: (results: AnalysisResult) => void;
  selectPosition: (position: number | null) => void;
  addAnnotation: (annotation: Omit<Annotation, 'id' | 'createdAt'>) => void;
  updateAnnotation: (id: string, updates: Partial<Pick<Annotation, 'label' | 'note' | 'color'>>) => void;
  removeAnnotation: (id: string) => void;
}


export const useProjectStore = create<ProjectState>((set, get) => ({
  currentProject: null,
  isLoadingProject: false,
  wizardStep: 'input',
  inputFilePath: null,
  copyInputFile: true,
  fastaValidation: null,
  headerFormatDetection: null,
  config: { ...DEFAULT_ANALYSIS_CONFIG },
  isAnalyzing: false,
  progress: null,
  analysisError: null,
  results: null,
  annotations: [],
  selectedPosition: null,
  layoutResetVersion: 0,
  bumpLayoutResetVersion: () => set((s) => ({ layoutResetVersion: s.layoutResetVersion + 1 })),

  createNewProject: async (name) => {
    const thisGen = ++projectOpenGeneration;
    // Clean up any in-flight state from the previous project
    get().closeProject();

    set({ isLoadingProject: true });
    try {
      const response = await createProject(name);
      if (thisGen !== projectOpenGeneration) return;
      set({
        currentProject: {
          name: response.name,
          path: response.path,
          createdAt: new Date().toISOString(),
          hasResults: false,
          hasInputFile: false,
        },
        config: { ...DEFAULT_ANALYSIS_CONFIG, queryName: name },
        wizardStep: 'input',
        results: null,
        annotations: [],
        inputFilePath: null,
        fastaValidation: null,
        headerFormatDetection: null,
        isAnalyzing: false,
        progress: null,
        analysisError: null,
        selectedPosition: null,
        isLoadingProject: false,
      });
      useAppStore.getState().refreshRecentProjects();
    } catch (error) {
      set({ isLoadingProject: false });
      console.error('Failed to create project:', error);
      throw error;
    }
  },

  openExistingProject: async (path) => {
    const thisGen = ++projectOpenGeneration;
    set({ isLoadingProject: true });
    try {
      // Load the new project data BEFORE closing the old one, so that
      // on failure the previous project is still intact in the store.
      const info = await openProject(path);
      // Discard if a newer open/create was initiated while this was in-flight
      if (thisGen !== projectOpenGeneration) return;
      
      let annotations: Annotation[] = [];
      try {
        annotations = await loadAnnotations(path);
      } catch (error) {
        showErrorToast('Failed to load annotations', error);
      }

      // Only now close the previous project (data loaded successfully)
      get().closeProject();

      const savedConfig = info.config as Partial<AnalysisConfig> | undefined;
      const restoredConfig = savedConfig
        ? { ...DEFAULT_ANALYSIS_CONFIG, ...savedConfig }
        : { ...DEFAULT_ANALYSIS_CONFIG };

      set({
        currentProject: {
          name: info.name,
          path: info.path,
          createdAt: info.created_at,
          hasResults: info.has_results,
          hasInputFile: info.has_input_file,
          inputFileName: info.input_file_name,
        },
        annotations,
        config: restoredConfig,
        // Construct the full path to the input file within the project directory.
        // The backend stores the file at project_path/file_name, so we need the
        // full path (not just the filename) for re-analysis to locate the file.
        inputFilePath: info.has_input_file && info.input_file_name
          ? `${info.path}/${info.input_file_name}`
          : null,
        // If we have results, go to results. If we have an input file, skip to
        // configure (the user already selected a file). Otherwise, start at input.
        wizardStep: info.has_results
          ? 'results'
          : info.has_input_file
            ? 'configure'
            : 'input',
        isLoadingProject: false,
        results: null,
        selectedPosition: null,
        analysisError: null,
      });
      useAppStore.getState().refreshRecentProjects();

      // Auto-trigger validation for projects with an input file but no results,
      // so the configure step has the fastaValidation data it needs. This runs
      // asynchronously and won't block the project open.
      if (info.has_input_file && info.input_file_name && !info.has_results) {
        const filePath = `${info.path}/${info.input_file_name}`;
        get().setInputFile(filePath).catch((e) => {
          console.error('Auto-validation of existing input file failed:', e);
        });
      }
    } catch (error) {
      set({ isLoadingProject: false });
      console.error('Failed to open project:', error);
      throw error;
    }
  },

  closeProject: () => {
    // 1. Flush pending annotation save (preserves user data)
    flushPendingAnnotationSave();

    // 2. Cancel backend analysis and validation if active
    if (get().isAnalyzing) {
      cancelAnalysis().catch((err) => showErrorToast('Failed to cancel analysis', err));
    }
    cancelValidation().catch(() => {});

    // 3. Bump generation to invalidate any in-flight async callbacks
    analysisGeneration++;
    validationGeneration++;

    // 4. Clear annotation save timer
    if (annotationSaveTimer) {
      clearTimeout(annotationSaveTimer);
      annotationSaveTimer = null;
    }

    // 5. Clear cross-store state
    useFeatureStore.getState().clearFeatures();

    // 6. Reset local state
    set({
      currentProject: null,
      wizardStep: 'input',
      inputFilePath: null,
      copyInputFile: true,
      fastaValidation: null,
      headerFormatDetection: null,
      config: { ...DEFAULT_ANALYSIS_CONFIG },
      isAnalyzing: false,
      progress: null,
      analysisError: null,
      results: null,
      annotations: [],
      selectedPosition: null,
    });
  },

  setWizardStep: (step) => {
    // Validate wizard transitions to prevent invalid states. (Fix 4.29)
    const { wizardStep: current, isAnalyzing, results } = get();
    const VALID_TRANSITIONS: Record<WizardStep, WizardStep[]> = {
      input: ['configure'],
      configure: ['input', 'analyzing'],
      analyzing: ['configure', 'results'],
      results: ['configure'],
    };
    if (step !== current && !VALID_TRANSITIONS[current]?.includes(step)) {
      console.warn(`Invalid wizard transition: ${current} → ${step}`);
      return;
    }
    if (step === 'results' && (!results || isAnalyzing)) {
      console.warn('Cannot transition to results without results data or during analysis');
      return;
    }
    set({ wizardStep: step });
  },

  goBack: () => {
    const { wizardStep, isAnalyzing } = get();
    if (wizardStep === 'configure') {
      set({ wizardStep: 'input' });
    } else if (wizardStep === 'analyzing') {
      // Cancel the running analysis before navigating back
      if (isAnalyzing) {
        get().cancelCurrentAnalysis();
      }
      set({ wizardStep: 'configure' });
    } else if (wizardStep === 'results') {
      set({ wizardStep: 'configure' });
    }
    // 'input' → no-op (already at the beginning)
  },

  goNext: async () => {
    const { wizardStep, fastaValidation } = get();
    if (wizardStep === 'input' && fastaValidation?.is_valid) {
      set({ wizardStep: 'configure' });
    } else if (wizardStep === 'configure') {
      await get().startAnalysis();
    }
  },

  setInputFile: async (path) => {
    // Cancel any in-flight validation so it doesn't consume CPU/IO for a file the user
    // no longer cares about. Fire-and-forget: we don't need to await the cancel IPC. (Fix 4.29)
    cancelValidation().catch(() => {});
    const currentGen = ++validationGeneration;
    set({ inputFilePath: path, fastaValidation: null, headerFormatDetection: null });
    
    let validation: FastaValidation;
    try {
      validation = await validateFasta(path);
      // Discard if a newer validation was started (rapid file changes)
      if (currentGen !== validationGeneration) return;
      set({ fastaValidation: validation });
    } catch (error) {
      if (currentGen !== validationGeneration) return;
      showErrorToast('Failed to validate file', error);
      set({ 
        fastaValidation: {
          is_valid: false,
          sequence_count: 0,
          sequence_length: null,
          sample_headers: [],
          detected_alphabet: 'unknown',
          errors: [{ error_type: 'validation_error', message: extractErrorMessage(error) ?? 'Validation failed unexpectedly', line_number: null }],
          file_size_bytes: 0,
          file_modified_at: null,
        },
      });
      return;
    }
    
    if (validation.is_valid) {
      // Only update alphabet if detection yielded a concrete type (Fix 5.61).
      // 'unknown' means the validator couldn't determine the type — keep the
      // existing config value (which defaults to 'protein') rather than passing
      // an invalid string to the backend.
      const detectedAlphabet: 'protein' | 'nucleotide' =
        validation.detected_alphabet === 'nucleotide' ? 'nucleotide' : 'protein';

      try {
        const detection = await detectHeaderFormat(path);
        if (currentGen !== validationGeneration) return;
        set({ 
          headerFormatDetection: detection,
          config: {
            ...get().config,
            alphabet: detectedAlphabet,
            headerFormat: detection.detected_format,
          },
        });
      } catch (headerError) {
        if (currentGen !== validationGeneration) return;
        showErrorToast('Failed to detect header format', headerError);
        set({
          config: {
            ...get().config,
            alphabet: detectedAlphabet,
          },
        });
      }
    }
  },

  setCopyInputFile: (copy) => {
    set({ copyInputFile: copy });
  },

  updateConfig: (partial) => {
    set((state) => ({
      config: { ...state.config, ...partial },
    }));
  },

  resetConfig: () => {
    set({ config: { ...DEFAULT_ANALYSIS_CONFIG } });
  },

  startAnalysis: async () => {
    // Re-entry guard: prevent double-invocation from rapid button clicks. (Fix 5.61)
    if (get().isAnalyzing) return;

    const { currentProject, inputFilePath, copyInputFile, config } = get();
    
    if (!currentProject || !inputFilePath) {
      set({ analysisError: 'Cannot start analysis: no project or input file selected.' });
      return;
    }

    const currentGen = ++analysisGeneration;

    set({ isAnalyzing: true, wizardStep: 'analyzing', progress: null, analysisError: null });

    try {
      const currentSettings = await getSettings();
      const updatedSettings = { ...currentSettings, lastUsedConfig: config };
      await updateSettings(updatedSettings);
      const { useSettingsStore } = await import('./settingsStore');
      useSettingsStore.setState({ settings: updatedSettings });
    } catch (error) {
      showErrorToast('Failed to save last used config', error);
    }

    let unlisten: UnlistenFn | null = null;
    let unlistenWarning: UnlistenFn | null = null;
    
    try {
      unlisten = await onAnalysisProgress((progress) => {
        if (currentGen === analysisGeneration) {
          set({ progress });
        }
      });

      // Surface non-fatal warnings from the backend (e.g. config save failure). (Fix 5.11)
      unlistenWarning = await onAnalysisWarning((message) => {
        showErrorToast(message);
      });

      // Normalize header_format and metadata_fields to pipe-delimited before
      // sending to backend — the core library only parses pipe ('|') delimiters,
      // even though the frontend may detect and display other delimiters (Fix 4.47)
      const normalizeDelimiter = (format: string | null): string | null => {
        if (!format) return null;
        return format.replace(/[\t,;]/g, '|');
      };

      // Pass file fingerprint from validation so the backend can detect
      // if the file changed between validation and analysis (TOCTOU binding).
      const { fastaValidation } = get();

      const response = await runAnalysis({
        project_path: currentProject.path,
        input_path: inputFilePath,
        copy_input: copyInputFile,
        kmer_length: config.kmerLength,
        support_threshold: config.supportThreshold,
        query_name: config.queryName || currentProject.name,
        alphabet: config.alphabet,
        header_format: normalizeDelimiter(config.headerFormat),
        metadata_fields: normalizeDelimiter(config.metadataFields),
        validation_mode: config.validationMode,
        allow_lowercase: config.allowLowercase,
        hcs_enabled: config.hcsEnabled,
        hcs_threshold: config.hcsThreshold,
        validated_file_size: fastaValidation?.file_size_bytes ?? null,
        validated_file_modified_at: fastaValidation?.file_modified_at ?? null,
      });

      if (currentGen !== analysisGeneration) return;

      // Surface any non-fatal warnings from the analysis (e.g., threshold > seq count).
      // These are informational and don't block success — shown as toast warnings.
      if (response.warnings && Array.isArray(response.warnings)) {
        for (const warning of response.warnings) {
          useToastStore.getState().addToast(warning, 'warning');
        }
      }

      try {
        const parsedResults = await loadResults(currentProject.path);
        if (currentGen !== analysisGeneration) return;
        
        set({ 
          isAnalyzing: false, 
          wizardStep: 'results',
          results: parsedResults,
          selectedPosition: null,
          currentProject: {
            ...currentProject,
            hasResults: true,
          },
        });
      } catch (loadError) {
        if (currentGen !== analysisGeneration) return;
        console.error('Failed to load results after analysis:', loadError);
        set({ 
          isAnalyzing: false, 
          analysisError: `Analysis completed but results could not be loaded: ${extractErrorMessage(loadError) ?? 'Unknown error'}`,
          wizardStep: 'analyzing',
        });
      }
    } catch (error) {
      if (currentGen !== analysisGeneration) return;
      console.error('Analysis failed:', error);
      set({ 
        isAnalyzing: false, 
        analysisError: extractErrorMessage(error) ?? 'Analysis failed unexpectedly. Check the console for details.',
        wizardStep: 'analyzing',
      });
    } finally {
      if (unlisten) unlisten();
      if (unlistenWarning) unlistenWarning();

      // Safety net: if this run was orphaned (generation advanced while we were
      // in-flight), ensure the UI doesn't stay stuck in the analyzing state.
      // The entity that bumped the generation SHOULD manage state, but if it
      // didn't reset isAnalyzing (e.g., due to its own error), this prevents a
      // permanently stuck UI.
      if (currentGen !== analysisGeneration && get().isAnalyzing) {
        set({ isAnalyzing: false, wizardStep: 'configure' });
      }
    }
  },

  cancelCurrentAnalysis: async () => {
    // Bump generation so any in-flight callbacks from the cancelled run are discarded.
    // Capture the new generation — if it advances again (user starts a new analysis
    // before this cancel IPC resolves), the finally block must NOT reset state.
    const cancelGen = ++analysisGeneration;
    try {
      await cancelAnalysis();
      // Provide user feedback that the cancellation was acknowledged (Fix 9.4.6)
      useToastStore.getState().addToast('Analysis cancelled', 'info');
    } catch (error) {
      showErrorToast('Failed to cancel analysis', error);
    } finally {
      // Only reset UI state if no newer analysis has started since this cancel
      if (analysisGeneration === cancelGen) {
        set({ 
          isAnalyzing: false, 
          progress: null, 
          analysisError: null,
          wizardStep: 'configure',
        });
      }
    }
  },

  setResults: (results) => {
    const { currentProject } = get();
    set({ 
      results,
      selectedPosition: null,
      ...(currentProject ? {
        currentProject: {
          ...currentProject,
          hasResults: true,
        },
      } : {}),
    });
  },

  selectPosition: (position) => {
    set({ selectedPosition: position });
  },

  addAnnotation: (annotationData) => {
    const { currentProject, annotations } = get();
    
    const MAX_ANNOTATIONS = 500;
    if (annotations.length >= MAX_ANNOTATIONS) {
      useToastStore.getState().addToast(`Maximum of ${MAX_ANNOTATIONS} annotations reached.`, 'warning');
      return;
    }

    const annotation: Annotation = {
      ...annotationData,
      id: crypto.randomUUID(),
      createdAt: new Date().toISOString(),
    };
    const newAnnotations = [...annotations, annotation];
    set({ annotations: newAnnotations });
    
    if (currentProject) {
      debouncedSaveAnnotations(currentProject.path, newAnnotations);
    }
  },

  updateAnnotation: (id, updates) => {
    const { currentProject, annotations } = get();
    const newAnnotations = annotations.map((a) =>
      a.id === id ? { ...a, ...updates } : a
    );
    set({ annotations: newAnnotations });

    if (currentProject) {
      debouncedSaveAnnotations(currentProject.path, newAnnotations);
    }
  },

  removeAnnotation: (id) => {
    const { currentProject, annotations } = get();
    const newAnnotations = annotations.filter((a) => a.id !== id);
    set({ annotations: newAnnotations });
    
    if (currentProject) {
      debouncedSaveAnnotations(currentProject.path, newAnnotations);
    }
  },
}));
