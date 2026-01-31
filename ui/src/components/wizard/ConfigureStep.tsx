/**
 * DiMA Desktop - Configure Step
 * 
 * Second wizard step for configuring analysis parameters.
 */

import { useState, useEffect, useRef } from 'react';
import { HelpCircle, ChevronLeft, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useProjectStore } from '@/stores/projectStore';
import { useSettingsStore } from '@/stores/settingsStore';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';

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
    short: 'Minimum support for entropy calculation (default: 30)',
    long: `Minimum support threshold for reliable entropy calculation.

"Support" is the number of valid k-mers at a given position across all sequences. This threshold affects both the entropy calculation method and the low-support classification labels in the output.

How support affects entropy calculation:
• support = 0: Entropy = 0.0 (no data)
• support = 1: Entropy = 0.0 (single k-mer, no diversity)
• support < threshold: Standard Shannon entropy
• support >= threshold: Extrapolation method using linear regression (more statistically robust for large samples)

Low-support classification labels in output:
• "NS" (No Support): support = 0 (no valid k-mers at position)
• "LS" (Low Support): support < threshold
• "ELS" (Exactly Low): support = threshold
• No label: support > threshold (normal)

Choosing a threshold:
• Default (30): Good balance for most datasets
• Lower (10-20): Use for smaller datasets (<100 sequences)
• Higher (50-100): Use for large datasets (>10,000 sequences)`,
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

// Help dialog component
function HelpDialog({ helpKey, onClose }: { helpKey: ParamHelpKey; onClose: () => void }) {
  const help = PARAM_HELP[helpKey];
  
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div 
        className="max-h-[80vh] w-full max-w-lg overflow-auto rounded-lg bg-background shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b px-6 py-4">
          <h2 className="text-lg font-semibold">{help.title}</h2>
          <button onClick={onClose} className="rounded-md p-2 hover:bg-muted">
            <X className="h-5 w-5" />
          </button>
        </div>
        <div className="p-6">
          <pre className="whitespace-pre-wrap font-sans text-sm leading-relaxed text-foreground">
            {help.long}
          </pre>
        </div>
      </div>
    </div>
  );
}

function ParamInput({ label, helpKey, children }: ParamInputProps) {
  const [showDialog, setShowDialog] = useState(false);
  const help = PARAM_HELP[helpKey];

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <label className="font-medium">{label}</label>
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger asChild>
              <button 
                onClick={() => setShowDialog(true)}
                className="text-muted-foreground hover:text-primary"
              >
                <HelpCircle className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right" className="max-w-xs">
              <p>{help.short}</p>
              <p className="mt-1 text-xs text-muted-foreground">Click for more details</p>
            </TooltipContent>
          </Tooltip>
        </TooltipProvider>
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
  } = useProjectStore();
  
  const { settings } = useSettingsStore();
  const loadedLastConfigRef = useRef(false);
  const [showAllowLowercaseHelp, setShowAllowLowercaseHelp] = useState(false);
  const [showHcsHelp, setShowHcsHelp] = useState(false);

  // Load last used config on mount (only once)
  useEffect(() => {
    if (loadedLastConfigRef.current) return;
    loadedLastConfigRef.current = true;
    
    // Apply last used config if available
    if (settings.lastUsedConfig) {
      updateConfig(settings.lastUsedConfig);
    } else {
      // Apply defaults from settings
      updateConfig({
        kmerLength: settings.defaultKmerLength,
        supportThreshold: settings.defaultSupportThreshold,
        validationMode: settings.defaultValidationMode,
      });
    }
  }, [settings, updateConfig]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="border-b px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-xl font-semibold">{currentProject?.name}</h1>
            <p className="text-sm text-muted-foreground">Step 2 of 3: Configure Analysis</p>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" onClick={goBack} className="gap-2">
              <ChevronLeft className="h-4 w-4" />
              Back
            </Button>
            <Button onClick={goNext}>
              Start Analysis
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
                  max={15}
                  value={config.kmerLength}
                  onChange={(e) => updateConfig({ kmerLength: Number(e.target.value) })}
                  className="w-full rounded-md border bg-background px-3 py-2"
                />
              </ParamInput>

              <ParamInput label="Support Threshold" helpKey="supportThreshold">
                <input
                  type="number"
                  min={1}
                  max={100}
                  value={config.supportThreshold}
                  onChange={(e) => updateConfig({ supportThreshold: Number(e.target.value) })}
                  className="w-full rounded-md border bg-background px-3 py-2"
                />
              </ParamInput>
            </div>

            <ParamInput label="Sequence Type" helpKey="alphabet">
              <div className="flex gap-4">
                <label className="flex items-center gap-2">
                  <input
                    type="radio"
                    checked={config.alphabet === 'protein'}
                    onChange={() => updateConfig({ alphabet: 'protein' })}
                    className="h-4 w-4"
                  />
                  Protein
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="radio"
                    checked={config.alphabet === 'nucleotide'}
                    onChange={() => updateConfig({ alphabet: 'nucleotide' })}
                    className="h-4 w-4"
                  />
                  Nucleotide
                </label>
              </div>
            </ParamInput>

            <ParamInput label="Validation Mode" helpKey="validationMode">
              <select
                value={config.validationMode}
                onChange={(e) => updateConfig({ validationMode: e.target.value as 'strict' | 'permissive' | 'report' })}
                className="w-full rounded-md border bg-background px-3 py-2"
              >
                <option value="strict">Strict</option>
                <option value="permissive">Permissive</option>
                <option value="report">Report</option>
              </select>
            </ParamInput>
          </section>

          {/* Header Format */}
          <section className="space-y-4">
            <h2 className="text-lg font-semibold">Header Format</h2>
            
            <ParamInput label="Header Format Pattern" helpKey="headerFormat">
              <div className="space-y-3">
                {/* Current format tags */}
                <div className="min-h-[40px] rounded-md border bg-background p-2">
                  {config.headerFormat ? (
                    <div className="flex flex-wrap gap-1">
                      {config.headerFormat.split(headerFormatDetection?.detected_delimiter || '|').map((field, i, arr) => (
                        <span key={i} className="flex items-center gap-1">
                          <button
                            type="button"
                            onClick={() => {
                              const fields = config.headerFormat?.split(headerFormatDetection?.detected_delimiter || '|') || [];
                              fields.splice(i, 1);
                              updateConfig({ headerFormat: fields.join(headerFormatDetection?.detected_delimiter || '|') || null });
                            }}
                            className="inline-flex items-center gap-1 rounded bg-primary/20 px-2 py-1 text-sm hover:bg-primary/30"
                          >
                            {field || `field${i + 1}`}
                            <span className="text-muted-foreground hover:text-foreground">&times;</span>
                          </button>
                          {i < arr.length - 1 && (
                            <span className="text-muted-foreground">{headerFormatDetection?.detected_delimiter || '|'}</span>
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
                            const delimiter = headerFormatDetection?.detected_delimiter || '|';
                            const current = config.headerFormat || '';
                            const newFormat = current ? `${current}${delimiter}${suggestion}` : suggestion;
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
                type="checkbox"
                checked={config.allowLowercase}
                onChange={(e) => updateConfig({ allowLowercase: e.target.checked })}
                className="mt-1 h-4 w-4 rounded border-gray-300"
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <p className="font-medium">Allow lowercase sequences</p>
                  <TooltipProvider>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <button 
                          onClick={() => setShowAllowLowercaseHelp(true)}
                          className="text-muted-foreground hover:text-primary"
                        >
                          <HelpCircle className="h-4 w-4" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="right" className="max-w-xs">
                        <p>{PARAM_HELP.allowLowercase.short}</p>
                        <p className="mt-1 text-xs text-muted-foreground">Click for more details</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
              </div>
            </div>

            <div className="flex items-start gap-3">
              <input
                type="checkbox"
                checked={config.hcsEnabled}
                onChange={(e) => updateConfig({ hcsEnabled: e.target.checked })}
                className="mt-1 h-4 w-4 rounded border-gray-300"
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <p className="font-medium">Calculate Highly Conserved Sequences (HCS)</p>
                  <TooltipProvider>
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <button 
                          onClick={() => setShowHcsHelp(true)}
                          className="text-muted-foreground hover:text-primary"
                        >
                          <HelpCircle className="h-4 w-4" />
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="right" className="max-w-xs">
                        <p>{PARAM_HELP.hcsEnabled.short}</p>
                        <p className="mt-1 text-xs text-muted-foreground">Click for more details</p>
                      </TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
              </div>
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
