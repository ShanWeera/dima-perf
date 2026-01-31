/**
 * DiMA Desktop - Project Store
 * 
 * Manages the current project state and analysis data.
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
import { 
  createProject, 
  openProject, 
  runAnalysis, 
  cancelAnalysis,
  onAnalysisProgress,
  validateFasta,
  detectHeaderFormat,
  saveAnnotations,
  loadAnnotations,
  updateSettings,
  getSettings,
} from '@/lib/tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';

export type WizardStep = 'input' | 'configure' | 'analyzing' | 'results';

interface ProjectState {
  // Current project
  currentProject: Project | null;
  
  // Wizard state
  wizardStep: WizardStep;
  
  // Input file
  inputFilePath: string | null;
  copyInputFile: boolean;
  fastaValidation: FastaValidation | null;
  headerFormatDetection: HeaderFormatDetection | null;
  
  // Analysis config
  config: AnalysisConfig;
  
  // Analysis state
  isAnalyzing: boolean;
  progress: ProgressUpdate | null;
  analysisError: string | null;
  
  // Results
  results: AnalysisResult | null;
  
  // Annotations
  annotations: Annotation[];
  
  // Selected position
  selectedPosition: number | null;

  // Actions - Project
  createNewProject: (name: string) => Promise<void>;
  openExistingProject: (path: string) => Promise<void>;
  closeProject: () => void;

  // Actions - Wizard
  setWizardStep: (step: WizardStep) => void;
  goBack: () => void;
  goNext: () => void;
  
  // Actions - Input
  setInputFile: (path: string) => Promise<void>;
  setCopyInputFile: (copy: boolean) => void;
  
  // Actions - Config
  updateConfig: (partial: Partial<AnalysisConfig>) => void;
  resetConfig: () => void;
  
  // Actions - Analysis
  startAnalysis: () => Promise<void>;
  cancelCurrentAnalysis: () => Promise<void>;
  
  // Actions - Results
  setResults: (results: AnalysisResult) => void;
  selectPosition: (position: number | null) => void;
  
  // Actions - Annotations
  addAnnotation: (annotation: Omit<Annotation, 'id' | 'createdAt'>) => void;
  removeAnnotation: (id: string) => void;
}

const DEFAULT_CONFIG: AnalysisConfig = {
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

export const useProjectStore = create<ProjectState>((set, get) => ({
  currentProject: null,
  wizardStep: 'input',
  inputFilePath: null,
  copyInputFile: true,
  fastaValidation: null,
  headerFormatDetection: null,
  config: { ...DEFAULT_CONFIG },
  isAnalyzing: false,
  progress: null,
  analysisError: null,
  results: null,
  annotations: [],
  selectedPosition: null,

  createNewProject: async (name) => {
    try {
      const response = await createProject(name);
      set({
        currentProject: {
          name: response.name,
          path: response.path,
          createdAt: new Date().toISOString(),
          hasResults: false,
          hasInputFile: false,
        },
        config: { ...DEFAULT_CONFIG, queryName: name },
        wizardStep: 'input',
        results: null,
        annotations: [],
        inputFilePath: null,
        fastaValidation: null,
      });
    } catch (error) {
      console.error('Failed to create project:', error);
      throw error;
    }
  },

  openExistingProject: async (path) => {
    try {
      const info = await openProject(path);
      
      // Load annotations for this project
      let annotations: Annotation[] = [];
      try {
        annotations = await loadAnnotations(path);
      } catch (error) {
        console.error('Failed to load annotations:', error);
      }

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
        wizardStep: info.has_results ? 'results' : 'input',
      });
    } catch (error) {
      console.error('Failed to open project:', error);
      throw error;
    }
  },

  closeProject: () => {
    set({
      currentProject: null,
      wizardStep: 'input',
      inputFilePath: null,
      copyInputFile: true,
      fastaValidation: null,
      headerFormatDetection: null,
      config: { ...DEFAULT_CONFIG },
      isAnalyzing: false,
      progress: null,
      analysisError: null,
      results: null,
      annotations: [],
      selectedPosition: null,
    });
  },

  setWizardStep: (step) => {
    set({ wizardStep: step });
  },

  goBack: () => {
    const { wizardStep } = get();
    if (wizardStep === 'configure') {
      set({ wizardStep: 'input' });
    }
  },

  goNext: () => {
    const { wizardStep, fastaValidation } = get();
    if (wizardStep === 'input' && fastaValidation?.is_valid) {
      set({ wizardStep: 'configure' });
    } else if (wizardStep === 'configure') {
      get().startAnalysis();
    }
  },

  setInputFile: async (path) => {
    set({ inputFilePath: path, fastaValidation: null, headerFormatDetection: null });
    
    try {
      // Validate the file
      const validation = await validateFasta(path);
      set({ fastaValidation: validation });
      
      // Detect header format
      if (validation.is_valid) {
        const detection = await detectHeaderFormat(path);
        set({ 
          headerFormatDetection: detection,
          config: {
            ...get().config,
            alphabet: validation.detected_alphabet as 'protein' | 'nucleotide',
            headerFormat: detection.detected_format,
          },
        });
      }
    } catch (error) {
      console.error('Failed to validate file:', error);
      set({ 
        fastaValidation: {
          is_valid: false,
          sequence_count: 0,
          sequence_length: null,
          sample_headers: [],
          detected_alphabet: 'unknown',
          errors: [{ error_type: 'error', message: String(error), line_number: null }],
          file_size_bytes: 0,
          file_modified_at: null,
        },
      });
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
    set({ config: { ...DEFAULT_CONFIG } });
  },

  startAnalysis: async () => {
    const { currentProject, inputFilePath, copyInputFile, config } = get();
    
    if (!currentProject || !inputFilePath) {
      return;
    }

    set({ isAnalyzing: true, wizardStep: 'analyzing', progress: null, analysisError: null });

    // Save config as last used config for future sessions
    try {
      const currentSettings = await getSettings();
      await updateSettings({
        ...currentSettings,
        lastUsedConfig: config,
      });
    } catch (error) {
      console.error('Failed to save last used config:', error);
    }

    // Set up progress listener
    let unlisten: UnlistenFn | null = null;
    
    try {
      unlisten = await onAnalysisProgress((progress) => {
        set({ progress });
      });

      const response = await runAnalysis({
        project_path: currentProject.path,
        input_path: inputFilePath,
        copy_input: copyInputFile,
        kmer_length: config.kmerLength,
        support_threshold: config.supportThreshold,
        query_name: config.queryName || currentProject.name,
        alphabet: config.alphabet,
        header_format: config.headerFormat,
        metadata_fields: config.metadataFields,
        validation_mode: config.validationMode,
        allow_lowercase: config.allowLowercase,
        hcs_enabled: config.hcsEnabled,
        hcs_threshold: config.hcsThreshold,
      });

      if (response.success) {
        // Load full results from file
        try {
          const { readTextFile } = await import('@tauri-apps/plugin-fs');
          const resultsPath = `${currentProject.path}/results.json`;
          const content = await readTextFile(resultsPath);
          const parsedResults = JSON.parse(content) as AnalysisResult;
          
          set({ 
            isAnalyzing: false, 
            wizardStep: 'results',
            results: parsedResults,
            currentProject: {
              ...currentProject,
              hasResults: true,
            },
          });
        } catch (loadError) {
          console.error('Failed to load results after analysis:', loadError);
          // Still navigate to results - ResultsStep will try to load
          set({ 
            isAnalyzing: false, 
            wizardStep: 'results',
            currentProject: {
              ...currentProject,
              hasResults: true,
            },
          });
        }
      }
    } catch (error) {
      console.error('Analysis failed:', error);
      set({ 
        isAnalyzing: false, 
        analysisError: String(error),
        wizardStep: 'configure',
      });
    } finally {
      if (unlisten) {
        unlisten();
      }
    }
  },

  cancelCurrentAnalysis: async () => {
    try {
      await cancelAnalysis();
      set({ 
        isAnalyzing: false, 
        progress: null, 
        wizardStep: 'configure',
      });
    } catch (error) {
      console.error('Failed to cancel analysis:', error);
    }
  },

  setResults: (results) => {
    set({ results });
  },

  selectPosition: (position) => {
    set({ selectedPosition: position });
  },

  addAnnotation: (annotationData) => {
    const { currentProject, annotations } = get();
    const annotation: Annotation = {
      ...annotationData,
      id: crypto.randomUUID(),
      createdAt: new Date().toISOString(),
    };
    const newAnnotations = [...annotations, annotation];
    set({ annotations: newAnnotations });
    
    // Persist to project
    if (currentProject) {
      saveAnnotations(currentProject.path, newAnnotations).catch((error) => {
        console.error('Failed to save annotations:', error);
      });
    }
  },

  removeAnnotation: (id) => {
    const { currentProject, annotations } = get();
    const newAnnotations = annotations.filter((a) => a.id !== id);
    set({ annotations: newAnnotations });
    
    // Persist to project
    if (currentProject) {
      saveAnnotations(currentProject.path, newAnnotations).catch((error) => {
        console.error('Failed to save annotations:', error);
      });
    }
  },
}));
