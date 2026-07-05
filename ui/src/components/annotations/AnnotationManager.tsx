/**
 * DiMA Desktop - Annotation Manager
 * 
 * Component for creating and managing position annotations.
 * Supports add, edit, delete, and navigating to annotated positions.
 */

import { useState, useEffect } from 'react';
import { Plus, Trash2, Pencil, MessageSquare, Check, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { Annotation, AnnotationColor } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';

const MAX_ANNOTATIONS = 500;

interface AnnotationManagerProps {
  annotations: Annotation[];
  selectedPosition: number | null;
  onAddAnnotation: (annotation: Omit<Annotation, 'id' | 'createdAt'>) => void;
  onUpdateAnnotation: (id: string, updates: Partial<Pick<Annotation, 'label' | 'note' | 'color'>>) => void;
  onRemoveAnnotation: (id: string) => void;
  onGoToPosition: (position: number) => void;
}

const COLOR_OPTIONS: AnnotationColor[] = [
  'red', 'orange', 'amber', 'yellow',
  'lime', 'green', 'teal', 'cyan',
  'blue', 'indigo', 'purple', 'pink',
];

export function AnnotationManager({
  annotations,
  selectedPosition,
  onAddAnnotation,
  onUpdateAnnotation,
  onRemoveAnnotation,
  onGoToPosition,
}: AnnotationManagerProps) {
  const [showAddForm, setShowAddForm] = useState(false);
  const [newColor, setNewColor] = useState<AnnotationColor>('blue');
  const [newLabel, setNewLabel] = useState('');
  const [newNote, setNewNote] = useState('');
  // Inline edit state
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editColor, setEditColor] = useState<AnnotationColor>('blue');
  const [editLabel, setEditLabel] = useState('');
  const [editNote, setEditNote] = useState('');

  // Reset form when selected position changes to prevent stale annotations
  useEffect(() => {
    setShowAddForm(false);
    setNewLabel('');
    setNewNote('');
    setEditingId(null);
  }, [selectedPosition]);

  const handleAdd = () => {
    if (selectedPosition === null) return;
    if (annotations.some((a) => a.positionNumber === selectedPosition)) return;

    onAddAnnotation({
      positionNumber: selectedPosition,
      color: newColor,
      label: newLabel,
      note: newNote,
    });

    setNewLabel('');
    setNewNote('');
    setShowAddForm(false);
  };

  const startEdit = (annotation: Annotation) => {
    setEditingId(annotation.id);
    setEditColor(annotation.color);
    setEditLabel(annotation.label || '');
    setEditNote(annotation.note || '');
  };

  const saveEdit = () => {
    if (!editingId) return;
    onUpdateAnnotation(editingId, { color: editColor, label: editLabel, note: editNote });
    setEditingId(null);
  };

  const cancelEdit = () => {
    setEditingId(null);
  };

  const isAtLimit = annotations.length >= MAX_ANNOTATIONS;

  // Get annotation for current position
  const currentAnnotation = annotations.find(
    (a) => a.positionNumber === selectedPosition
  );

  return (
    <div className="flex h-full flex-col">
      {/* Header with count indicator */}
      <div className="flex items-center justify-between border-b px-4 py-3">
        <div className="flex items-center gap-2">
          <h3 className="font-semibold">Annotations</h3>
          <span className="text-xs text-muted-foreground">
            {annotations.length}/{MAX_ANNOTATIONS}
          </span>
        </div>
        {selectedPosition !== null && !currentAnnotation && !showAddForm && (
          <Button
            size="sm"
            variant="outline"
            onClick={() => setShowAddForm(true)}
            disabled={isAtLimit}
            title={isAtLimit ? `Maximum of ${MAX_ANNOTATIONS} annotations reached` : undefined}
            className="gap-1"
          >
            <Plus className="h-3 w-3" />
            Add
          </Button>
        )}
      </div>

      {/* Add Form */}
      {showAddForm && selectedPosition !== null && (
        <div className="border-b p-4 space-y-3">
          <p className="text-sm font-medium">
            Annotate Position {selectedPosition}
          </p>

          {/* Color Picker */}
          <div className="flex flex-wrap gap-1">
            {COLOR_OPTIONS.map((color) => (
              <button
                key={color}
                onClick={() => setNewColor(color)}
                aria-label={`Select ${color} color`}
                aria-pressed={newColor === color}
                className={`h-6 w-6 rounded-full transition-transform ${
                  newColor === color ? 'scale-125 ring-2 ring-offset-2' : ''
                }`}
                style={{ backgroundColor: ANNOTATION_COLORS[color] }}
              />
            ))}
          </div>

          {/* Label */}
          <input
            type="text"
            placeholder="Label (optional)"
            value={newLabel}
            onChange={(e) => setNewLabel(e.target.value)}
            className="w-full rounded-md border bg-background px-3 py-2 text-sm"
          />

          {/* Note */}
          <textarea
            placeholder="Note..."
            value={newNote}
            onChange={(e) => setNewNote(e.target.value)}
            className="w-full rounded-md border bg-background px-3 py-2 text-sm"
            rows={3}
          />

          {/* Actions */}
          <div className="flex justify-end gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() => setShowAddForm(false)}
            >
              Cancel
            </Button>
            <Button size="sm" onClick={handleAdd}>
              Add Annotation
            </Button>
          </div>
        </div>
      )}

      {/* Annotations List */}
      <div className="flex-1 overflow-auto p-2">
        {annotations.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-8 text-center text-muted-foreground">
            <MessageSquare className="mb-2 h-8 w-8" />
            <p className="text-sm">No annotations yet</p>
            <p className="text-xs">Select a position and click Add</p>
          </div>
        ) : (
          <div className="space-y-2">
            {annotations.map((annotation) => (
              <div
                key={annotation.id}
                className={`rounded-lg border p-3 transition-colors ${
                  annotation.positionNumber === selectedPosition
                    ? 'border-primary bg-primary/5'
                    : ''
                } ${editingId === annotation.id ? '' : 'cursor-pointer hover:bg-muted'}`}
                onClick={editingId === annotation.id ? undefined : () => onGoToPosition(annotation.positionNumber)}
              >
                {editingId === annotation.id ? (
                  // Inline edit mode
                  <div className="space-y-2">
                    <p className="text-sm font-medium">Position {annotation.positionNumber}</p>
                    <div className="flex flex-wrap gap-1">
                      {COLOR_OPTIONS.map((color) => (
                        <button
                          key={color}
                          onClick={() => setEditColor(color)}
                          aria-label={`Select ${color} color`}
                          aria-pressed={editColor === color}
                          className={`h-5 w-5 rounded-full transition-transform ${
                            editColor === color ? 'scale-125 ring-2 ring-offset-1' : ''
                          }`}
                          style={{ backgroundColor: ANNOTATION_COLORS[color] }}
                        />
                      ))}
                    </div>
                    <input
                      type="text"
                      placeholder="Label (optional)"
                      value={editLabel}
                      onChange={(e) => setEditLabel(e.target.value)}
                      className="w-full rounded-md border bg-background px-2 py-1 text-sm"
                    />
                    <textarea
                      placeholder="Note..."
                      value={editNote}
                      onChange={(e) => setEditNote(e.target.value)}
                      className="w-full rounded-md border bg-background px-2 py-1 text-sm"
                      rows={2}
                    />
                    <div className="flex justify-end gap-1">
                      <Button size="sm" variant="ghost" onClick={cancelEdit} className="h-7 px-2">
                        <X className="h-3 w-3" />
                      </Button>
                      <Button size="sm" onClick={saveEdit} className="h-7 px-2">
                        <Check className="h-3 w-3" />
                      </Button>
                    </div>
                  </div>
                ) : (
                  // Display mode
                  <>
                    <div className="flex items-start justify-between">
                      <div className="flex items-center gap-2">
                        <div
                          className="h-3 w-3 rounded-full flex-shrink-0"
                          style={{ backgroundColor: ANNOTATION_COLORS[annotation.color] }}
                        />
                        <span className="text-sm font-medium">
                          Position {annotation.positionNumber}
                        </span>
                        {annotation.label && (
                          <span className="truncate text-xs text-muted-foreground" title={annotation.label}>
                            {annotation.label}
                          </span>
                        )}
                      </div>
                      <div className="flex items-center gap-1">
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            startEdit(annotation);
                          }}
                          className="text-muted-foreground hover:text-foreground"
                          aria-label={`Edit annotation${annotation.label ? `: ${annotation.label}` : ''}`}
                        >
                          <Pencil className="h-3.5 w-3.5" />
                        </button>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            onRemoveAnnotation(annotation.id);
                          }}
                          className="text-muted-foreground hover:text-destructive"
                          aria-label={`Delete annotation${annotation.label ? `: ${annotation.label}` : ''}`}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                        </button>
                      </div>
                    </div>
                    {annotation.note && (
                      <p className="mt-1 text-xs text-muted-foreground line-clamp-2">
                        {annotation.note}
                      </p>
                    )}
                  </>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
