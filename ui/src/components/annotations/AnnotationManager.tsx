/**
 * DiMA Desktop - Annotation Manager
 * 
 * Component for creating and managing position annotations.
 */

import { useState } from 'react';
import { Plus, Trash2, MessageSquare } from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { Annotation, AnnotationColor } from '@/lib/types';
import { ANNOTATION_COLORS } from '@/lib/types';

interface AnnotationManagerProps {
  annotations: Annotation[];
  selectedPosition: number | null;
  onAddAnnotation: (annotation: Omit<Annotation, 'id' | 'createdAt'>) => void;
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
  onRemoveAnnotation,
  onGoToPosition,
}: AnnotationManagerProps) {
  const [showAddForm, setShowAddForm] = useState(false);
  const [newColor, setNewColor] = useState<AnnotationColor>('blue');
  const [newLabel, setNewLabel] = useState('');
  const [newNote, setNewNote] = useState('');

  const handleAdd = () => {
    if (selectedPosition === null) return;

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

  // Get annotation for current position
  const currentAnnotation = annotations.find(
    (a) => a.positionNumber === selectedPosition
  );

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b px-4 py-3">
        <h3 className="font-semibold">Annotations</h3>
        {selectedPosition !== null && !currentAnnotation && (
          <Button
            size="sm"
            variant="outline"
            onClick={() => setShowAddForm(true)}
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
                onClick={() => onGoToPosition(annotation.positionNumber)}
                className={`cursor-pointer rounded-lg border p-3 transition-colors hover:bg-muted ${
                  annotation.positionNumber === selectedPosition
                    ? 'border-primary bg-primary/5'
                    : ''
                }`}
              >
                <div className="flex items-start justify-between">
                  <div className="flex items-center gap-2">
                    <div
                      className="h-3 w-3 rounded-full"
                      style={{ backgroundColor: ANNOTATION_COLORS[annotation.color] }}
                    />
                    <span className="text-sm font-medium">
                      Position {annotation.positionNumber}
                    </span>
                    {annotation.label && (
                      <span className="text-xs text-muted-foreground">
                        {annotation.label}
                      </span>
                    )}
                  </div>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemoveAnnotation(annotation.id);
                    }}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>
                {annotation.note && (
                  <p className="mt-1 text-xs text-muted-foreground line-clamp-2">
                    {annotation.note}
                  </p>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
