/**
 * DiMA Desktop - Configure Step
 * 
 * Second wizard step for configuring analysis parameters.
 */

import { useState, useEffect, useRef } from 'react';
import { HelpCircle, ChevronLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { useShallow } from 'zustand/react/shallow';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog';

// Parameter help text from CLI (comprehensive)
const PARAM_HELP = {
  kmerLength: {
    title: 'K-mer Length',
    short: 'Size of the sliding window (default: 9)',
    long: `K-mer length for sliding window analysis.

The k-mer length determines the size of the sliding window used to extract overlapping subsequences from each aligned sequence. For a sequence of length L, this generates (L - k + 1) k-mers at positions 0, 1, 2, ..., L-k.

How it works:
• A sliding window of size k moves across each sequence
• At each position, the k-mer is extracted and encoded
• K-mers are compared across all sequences at the same position
• Entropy is calculated based on k-mer diversity at each position

Recommended values:
• k=9 (default): Standard for T-cell epitope analysis (nonamers)
• k=8-11: Common range for immune epitope studies
• k=3-6: Short motif discovery
• k=15-20: High-specificity sequence matching

Technical limits (due to u64 encoding):
• Protein sequences: maximum k ~ 13-14
• Nucleotide sequences: maximum k ~ 27-28`,
  },
  supportThreshold: {
    title: 'Support Threshold',
    short: 'Minimum support for entropy calculation (default: 100, per PMC11596295)',
    long: `Minimum support threshold for reliable entropy calculation.

"Support" is the number of valid k-mers at a given position across all sequences. This threshold affects both the entropy calculation method and the low-support classification labels in the output.

How support affects entropy calculation:
• support = 0: Entropy = 0.0 (no data)
• support = 1: Entropy = 0.0 (single k-mer, no diversity)
• support < threshold: Standard Shannon entropy
• support > threshold: Extrapolation method using linear regression (more statistically robust for large samples)

Low-support classification labels in output:
• "NS" (No Support): support = 0 (no valid k-mers at position)
• "LS" (Low Support): support < threshold
• "ELS" (Exceptional Low Support): support = threshold
• No label: support > threshold (normal)

Choosing a threshold:
• Default (100): Per the DiMA paper (PMC11596295), 100 provides a robust baseline
• Lower (10-50): Use for smaller datasets (<100 sequences)
• Higher (200+): Use for very large datasets (>10,000 sequences)`,
  },
  alphabet: {
    title: 'Sequence Type',
    short: 'Sequence type: protein or nucleotide',
    long: `Specify the sequence alphabet type for validation and encoding.

DiMA supports two alphabet types, each with different valid characters and encoding schemes:

PROTEIN (default):
• Valid characters: A C D E F G H I K L M N P Q R S T V W Y (20 amino acids)
• Ambiguous codes: X (any), B (D/N), J (L/I), Z (E/Q), O (pyrrolysine), U (selenocysteine)
• K-mer encoding: Base-20 arithmetic (max k ~ 13-14)
• Use for: Protein sequence diversity analysis

NUCLEOTIDE:
• Valid characters: A C G T U (DNA + RNA bases)
• Ambiguous codes: R Y K M S W B D H V N (IUPAC ambiguity codes)
• K-mer encoding: Base-5 arithmetic (max k ~ 27-28)
• Use for: DNA/RNA sequence diversity analysis

The alphabet choice affects:
1. Character validation (what's considered valid/invalid)
2. K-mer encoding efficiency (base used for numeric encoding)
3. Maximum practical k-mer length (due to u64 overflow)`,
  },
  validationMode: {
    title: 'Validation Mode',
    short: 'How to handle invalid characters',
    long: `Character validation mode for k-mer generation.

Controls how invalid and ambiguous characters in sequences are handled.

STRICT (default) - Recommended for scientific accuracy:
Valid characters only. Any other character invalidates the k-mer.

• Protein whitelist: A C D E F G H I K L M N P Q R S T V W Y (20 canonical amino acids)
• Nucleotide whitelist: A C G T U (DNA + RNA bases)
• Ambiguous codes (X, B, N, etc.) → k-mer marked as NA
• Invalid chars (#, *, @, numbers) → k-mer marked as NA

PERMISSIVE - Accept ambiguous IUPAC codes:
Valid + ambiguous characters accepted. Only completely invalid characters cause k-mer invalidation.

• Protein ambiguous: X (any), B (D/N), J (L/I), Z (E/Q), O, U
• Nucleotide ambiguous: R Y K M S W B D H V N (IUPAC codes)
• Use when: sequences contain standard ambiguity notation

REPORT - Data quality assessment:
All characters accepted (no k-mer invalidation). Statistics are tracked for reporting.
Use when: exploring unknown data quality`,
  },
  headerFormat: {
    title: 'Header Format',
    short: 'Pattern for parsing sequence headers',
    long: `Define the pipe-separated format of FASTA header metadata.

FASTA headers often contain metadata separated by pipe '|' characters. This option tells DiMA how to parse and name each field for aggregation.

If not provided, metadata processing is disabled entirely:
• FASTA headers are ignored (only sequences are processed)
• No metadata aggregation per variant
• Variants have "metadata": null in output
• Faster processing and lower memory usage

Format specification:
• Fields are separated by '|' in the format string
• The format MUST match the number of '|' separators in headers
• Whitespace around field names is automatically trimmed
• Field names become keys in the metadata aggregation output

Example FASTA header:
  >USA|2023-01-15|Patient001|Delta

Matching format:
  country|date|patient_id|variant

This produces metadata aggregation per k-mer variant showing counts for each unique value.`,
  },
  allowLowercase: {
    title: 'Allow Lowercase',
    short: 'Convert lowercase letters to uppercase',
    long: `Allow and convert lowercase characters in sequences.

By default, lowercase letters (a-z) are treated as invalid characters and cause k-mers containing them to be marked as NA.

When enabled:
• Lowercase letters are converted to uppercase during encoding
• 'a' → 'A', 'c' → 'C', etc.
• No performance penalty after initialization

When disabled (default):
• Lowercase letters are classified as Invalid
• K-mers containing lowercase are marked as NA
• Ensures input data quality (catches unexpected formatting)

Use cases for enabling:
• Input files use mixed case (common in some databases)
• Sequences use lowercase for masking (soft-masked regions)
• Converting legacy data with inconsistent formatting`,
  },
  hcsEnabled: {
    title: 'Highly Conserved Sequences (HCS)',
    short: 'Identify stretches of conserved k-mers',
    long: `Calculate Highly Conserved Sequences.

HCS (Highly Conserved Sequences) are regions where the same k-mer appears most frequently across all sequences. These are extracted from variants classified as "Index" (motif_short = "I").

How Index variants are classified:
• Must have the highest count at that position
• Count must be > 1 (not unique)
• Represents the "consensus" or most conserved sequence

HCS extraction algorithm:
1. Find all Index variants at each position
2. Concatenate overlapping k-mers (suffix-prefix matching)
3. If no overlap, start a new conserved region
4. Output as sequence strings with position ranges

Use cases:
• Identify conserved epitopes for vaccine design
• Find evolutionarily stable protein regions
• Extract consensus sequences from alignments`,
  },
};

type ParamHelpKey = keyof typeof PARAM_HELP;

interface ParamInputProps {
  label: string;
  helpKey: ParamHelpKey;
  children: React.ReactNode;
}

// Help dialog component using shadcn Dialog for proper focus trapping and accessibility
function HelpDialog({ helpKey, onClose }: { helpKey: ParamHelpKey; onClose: () => void }) {
  const help = PARAM_HELP[helpKey];
  
  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="max-h-[80vh] max-w-lg overflow-auto">
        <DialogHeader>
          <DialogTitle>{help.title}</DialogTitle>
          <DialogDescription className="sr-only">
            Help information for the {help.title} parameter
          </DialogDescription>
        </DialogHeader>
        <pre className="whitespace-pre-wrap font-sans text-sm leading-relaxed text-foreground">
          {help.long}
        </pre>
      </DialogContent>
    </Dialog>
  );
}

function ParamInput({ label, helpKey, children }: ParamInputProps) {
  const [showDialog, setShowDialog] = useState(false);
  const help = PARAM_HELP[helpKey];

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <label className="font-medium">{label}</label>
        <Tooltip>
            <TooltipTrigger asChild>
              <button 
                onClick={() => setShowDialog(true)}
                className="text-muted-foreground hover:text-primary"
                aria-label="Help: Support threshold"
              >
                <HelpCircle className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right" className="max-w-xs">
              <p>{help.short}</p>
              <p className="mt-1 text-xs text-muted-foreground">Click for more details</p>
            </TooltipContent>
          </Tooltip>
      </div>
      {children}
      {showDialog && <HelpDialog helpKey={helpKey} onClose={() => setShowDialog(false)} />}
    </div>
  );
}

export function ConfigureStep() {
  const { 
    currentProject,
    config, 
    updateConfig, 
    goBack, 
    goNext,
    headerFormatDetection,
    isAnalyzing,
  } = useProjectStore(useShallow((s) => ({
    currentProject: s.currentProject,
    config: s.config,
    updateConfig: s.updateConfig,
    goBack: s.goBack,
    goNext: s.goNext,
    headerFormatDetection: s.headerFormatDetection,
    isAnalyzing: s.isAnalyzing,
  })));
  
  const { settings, isInitialized: settingsReady } = useSettingsStore();
  const appliedConfigForProject = useRef<string | null>(null);
  const [showAllowLowercaseHelp, setShowAllowLowercaseHelp] = useState(false);
  const [showHcsHelp, setShowHcsHelp] = useState(false);

  // Apply lastUsedConfig ONLY on first mount for a new project that hasn't been
  // configured yet. Uses the project path as a key to ensure we never re-apply
  // after navigation within the same project (Back → Configure).
  // Only applies when the config is still at default values (kmerLength === 9,
  // supportThreshold === 100) — if the project was opened with a saved config,
  // the defaults would already have been overwritten by openExistingProject.
  // Waits for headerFormatDetection to resolve so we don't clobber detected values.
  useEffect(() => {
    if (!settingsReady || !currentProject?.path) return;
    if (appliedConfigForProject.current === currentProject.path) return;

    // Defer until header format detection is available (or confirmed unavailable).
    // This prevents the race where lastUsedConfig overwrites a detected format
    // that hasn't arrived yet during project reopen auto-validation.
    if (headerFormatDetection === null && config.headerFormat === null) {
      // Detection still in-flight — skip for now, re-run when it arrives
      return;
    }

    appliedConfigForProject.current = currentProject.path;

    // Only apply lastUsedConfig when the project config is still at defaults,
    // indicating it hasn't been configured by the user or restored from disk.
    const isDefaultConfig = config.kmerLength === 9 && config.supportThreshold === 100;
    if (!isDefaultConfig) return;

    const detectedAlphabet = config.alphabet;
    const detectedHeaderFormat = config.headerFormat;

    if (settings.lastUsedConfig) {
      const maxKForDetected = detectedAlphabet === 'protein' ? 14 : 27;
      updateConfig({
        ...settings.lastUsedConfig,
        // Preserve detected values from file validation rather than
        // overwriting with stale lastUsedConfig values
        alphabet: detectedAlphabet,
        headerFormat: detectedHeaderFormat,
        // Clamp kmerLength to the max for the detected alphabet (not the
        // lastUsedConfig's alphabet) to prevent encoding overflow (Fix 5.41)
        kmerLength: Math.min(settings.lastUsedConfig.kmerLength ?? 9, maxKForDetected),
      });
    } else {
      const maxKForDetected = detectedAlphabet === 'protein' ? 14 : 27;
      updateConfig({
        kmerLength: Math.min(settings.defaultKmerLength, maxKForDetected),
        supportThreshold: settings.defaultSupportThreshold,
        validationMode: settings.defaultValidationMode,
      });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settingsReady, currentProject?.path, headerFormatDetection]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold truncate min-w-0">{currentProject?.name}</h1>
            <p className="text-sm text-muted-foreground">Step 2 of 3: Configure Analysis</p>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={goBack} className="gap-2">
              <ChevronLeft className="h-4 w-4" />
              Back
            </Button>
            <Button 
              onClick={goNext}
              disabled={isAnalyzing || !Number.isFinite(config.kmerLength) || config.kmerLength < 3 || config.kmerLength > (config.alphabet === 'protein' ? 14 : 27) || !Number.isFinite(config.supportThreshold) || config.supportThreshold < 1}
            >
              {isAnalyzing ? 'Starting...' : 'Start Analysis'}
            </Button>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        <div className="mx-auto max-w-2xl space-y-6">
          {/* Basic Parameters */}
          <section className="space-y-4">
            <h2 className="text-lg font-semibold">Analysis Parameters</h2>
            
            <div className="grid grid-cols-2 gap-4">
              <ParamInput label="K-mer Length" helpKey="kmerLength">
                <input
                  type="number"
                  min={3}
                  max={config.alphabet === 'nucleotide' ? 27 : 14}
                  value={config.kmerLength}
                  onChange={(e) => {
                    const val = parseInt(e.target.value, 10);
                    if (!Number.isNaN(val) && val >= 3) {
                      const maxK = config.alphabet === 'nucleotide' ? 27 : 14;
                      updateConfig({ kmerLength: Math.min(val, maxK) });
                    }
                  }}
                  className="w-full rounded-md border bg-background px-3 py-2"
                />
              </ParamInput>

              <ParamInput label="Support Threshold" helpKey="supportThreshold">
                <input
                  type="number"
                  min={1}
                  max={10000}
                  value={config.supportThreshold}
                  onChange={(e) => {
                    const val = parseInt(e.target.value, 10);
                    if (!Number.isNaN(val) && val >= 1) {
                      updateConfig({ supportThreshold: Math.min(val, 10000) });
                    }
                  }}
                  className="w-full rounded-md border bg-background px-3 py-2"
                />
              </ParamInput>
            </div>

            <ParamInput label="Sequence Type" helpKey="alphabet">
              <RadioGroup
                value={config.alphabet}
                onValueChange={(value: string) => {
                  const alphabet = value as 'protein' | 'nucleotide';
                  const maxK = alphabet === 'protein' ? 14 : 27;
                  const update: Partial<typeof config> = { alphabet };
                  if (config.kmerLength > maxK) update.kmerLength = maxK;
                  updateConfig(update);
                }}
                className="flex gap-4"
              >
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="protein" id="alphabet-protein" />
                  <Label htmlFor="alphabet-protein" className="font-normal cursor-pointer">Protein</Label>
                </div>
                <div className="flex items-center gap-2">
                  <RadioGroupItem value="nucleotide" id="alphabet-nucleotide" />
                  <Label htmlFor="alphabet-nucleotide" className="font-normal cursor-pointer">Nucleotide</Label>
                </div>
              </RadioGroup>
            </ParamInput>

            <ParamInput label="Validation Mode" helpKey="validationMode">
              <Select value={config.validationMode} onValueChange={(v) => updateConfig({ validationMode: v as 'strict' | 'permissive' | 'report' })}>
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="strict">Strict</SelectItem>
                  <SelectItem value="permissive">Permissive</SelectItem>
                  <SelectItem value="report">Report</SelectItem>
                </SelectContent>
              </Select>
            </ParamInput>
          </section>

          {/* Header Format */}
          <section className="space-y-4">
            <h2 className="text-lg font-semibold">Header Format</h2>
            
            <ParamInput label="Header Format Pattern" helpKey="headerFormat">
              <div className="space-y-3">
                {/* Current format tags — always use '|' as internal delimiter (Fix 5.97).
                   Backend normalizes all delimiters to '|' at analysis time, so the UI
                   should consistently display/edit with '|' regardless of detection result. */}
                <div className="min-h-[40px] rounded-md border bg-background p-2">
                  {config.headerFormat ? (
                    <div className="flex flex-wrap gap-1">
                      {config.headerFormat.split(/[|\t,;]/).map((field, i, arr) => (
                        <span key={i} className="flex items-center gap-1">
                          <button
                            type="button"
                            onClick={() => {
                              const fields = config.headerFormat?.split(/[|\t,;]/) || [];
                              fields.splice(i, 1);
                              updateConfig({ headerFormat: fields.join('|') || null });
                            }}
                            className="inline-flex items-center gap-1 rounded bg-primary/20 px-2 py-1 text-sm hover:bg-primary/30"
                          >
                            {field || `field${i + 1}`}
                            <span className="text-muted-foreground hover:text-foreground">&times;</span>
                          </button>
                          {i < arr.length - 1 && (
                            <span className="text-muted-foreground">|</span>
                          )}
                        </span>
                      ))}
                    </div>
                  ) : (
                    <span className="text-sm text-muted-foreground">Click suggested fields below or type custom format</span>
                  )}
                </div>

                {/* Manual input */}
                <input
                  type="text"
                  value={config.headerFormat || ''}
                  onChange={(e) => updateConfig({ headerFormat: e.target.value || null })}
                  placeholder="e.g., accession|country|date"
                  className="w-full rounded-md border bg-background px-3 py-2 font-mono text-sm"
                />

                {/* Suggested field names */}
                {headerFormatDetection && headerFormatDetection.field_count > 0 && (
                  <div>
                    <p className="mb-2 text-xs text-muted-foreground">Click to add suggested fields:</p>
                    <div className="flex flex-wrap gap-1">
                      {['accession', 'isolate', 'country', 'date', 'host', 'lineage', 'clade', 'subtype', 'segment'].map((suggestion) => (
                        <button
                          key={suggestion}
                          type="button"
                          onClick={() => {
                            const current = config.headerFormat || '';
                            const newFormat = current ? `${current}|${suggestion}` : suggestion;
                            updateConfig({ headerFormat: newFormat });
                          }}
                          className="rounded bg-muted px-2 py-1 text-xs hover:bg-accent"
                        >
                          + {suggestion}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            </ParamInput>

            {headerFormatDetection && headerFormatDetection.sample_parsed.length > 0 && (
              <div>
                <p className="mb-2 text-sm font-medium">Preview</p>
                <div className="space-y-2 rounded-lg bg-muted p-3 text-xs">
                  {headerFormatDetection.sample_parsed.map((parsed, i) => (
                    <div key={i} className="space-y-1">
                      <p className="font-mono text-muted-foreground truncate">
                        {'>'}{parsed.raw}
                      </p>
                      <div className="flex flex-wrap gap-1">
                        {parsed.fields.map((field, j) => (
                          <span 
                            key={j} 
                            className="inline-block rounded bg-primary/10 px-2 py-0.5"
                          >
                            {field || '(empty)'}
                          </span>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </section>

          {/* Advanced Options */}
          <section className="space-y-4">
            <h2 className="text-lg font-semibold">Advanced Options</h2>
            
            <div className="flex items-start gap-3">
              <input
                id="config-allow-lowercase"
                type="checkbox"
                checked={config.allowLowercase}
                onChange={(e) => updateConfig({ allowLowercase: e.target.checked })}
                className="mt-1 h-4 w-4 rounded border-input"
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <label htmlFor="config-allow-lowercase" className="font-medium cursor-pointer">Allow lowercase sequences</label>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <button 
                        onClick={() => setShowAllowLowercaseHelp(true)}
                        className="text-muted-foreground hover:text-primary"
                        aria-label="Help: Allow lowercase"
                      >
                        <HelpCircle className="h-4 w-4" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent side="right" className="max-w-xs">
                      <p>{PARAM_HELP.allowLowercase.short}</p>
                      <p className="mt-1 text-xs text-muted-foreground">Click for more details</p>
                    </TooltipContent>
                  </Tooltip>
                </div>
              </div>
            </div>

            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span>Highly Conserved Sequences (HCS) are always calculated from Index motifs.</span>
              <Tooltip>
                <TooltipTrigger asChild>
                  <button 
                    onClick={() => setShowHcsHelp(true)}
                    className="text-muted-foreground hover:text-primary"
                    aria-label="Help: HCS calculation"
                  >
                    <HelpCircle className="h-4 w-4" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="right" className="max-w-xs">
                  <p>{PARAM_HELP.hcsEnabled.short}</p>
                  <p className="mt-1 text-xs text-muted-foreground">Click for more details</p>
                </TooltipContent>
              </Tooltip>
            </div>
          </section>

          {/* Help Dialogs for Advanced Options */}
          {showAllowLowercaseHelp && (
            <HelpDialog helpKey="allowLowercase" onClose={() => setShowAllowLowercaseHelp(false)} />
          )}
          {showHcsHelp && (
            <HelpDialog helpKey="hcsEnabled" onClose={() => setShowHcsHelp(false)} />
          )}
        </div>
      </div>
    </div>
  );
}
