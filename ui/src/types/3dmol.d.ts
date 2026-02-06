// Type declarations for 3dmol.js
declare module '3dmol' {
  export interface ViewerSpec {
    backgroundColor?: string;
    antialias?: boolean;
    cartoonQuality?: number;
  }

  export interface AtomSelectionSpec {
    chain?: string;
    resi?: number | number[] | string;
    atom?: string;
    elem?: string;
    hetflag?: boolean;
    serial?: number;
    model?: number;
  }

  export interface CartoonStyleSpec {
    color?: string;
    style?: 'trace' | 'tube' | 'ribbon' | 'arrow' | 'rectangle' | 'parabola' | 'edgedStrand';
    thickness?: number;
    opacity?: number;
  }

  export interface StickStyleSpec {
    color?: string;
    radius?: number;
    singleBonds?: boolean;
    opacity?: number;
  }

  export interface SphereStyleSpec {
    color?: string;
    radius?: number;
    opacity?: number;
  }

  export interface LineStyleSpec {
    color?: string;
    linewidth?: number;
    opacity?: number;
  }

  export interface SurfaceStyleSpec {
    color?: string;
    opacity?: number;
    colorscheme?: string;
  }

  export interface AtomStyleSpec {
    cartoon?: CartoonStyleSpec;
    stick?: StickStyleSpec;
    sphere?: SphereStyleSpec;
    line?: LineStyleSpec;
    surface?: SurfaceStyleSpec;
  }

  export interface LabelSpec {
    font?: string;
    fontSize?: number;
    fontColor?: string;
    fontOpacity?: number;
    borderThickness?: number;
    borderColor?: string;
    borderOpacity?: number;
    backgroundColor?: string;
    backgroundOpacity?: number;
    position?: { x: number; y: number; z: number };
    inFront?: boolean;
    showBackground?: boolean;
    alignment?: string;
  }

  export interface GLModel {
    setStyle(sel: AtomSelectionSpec, style: AtomStyleSpec): void;
    getAtoms(sel?: AtomSelectionSpec): Atom[];
    removeAllLabels(): void;
  }

  export interface Atom {
    serial: number;
    atom: string;
    elem: string;
    chain: string;
    resi: number;
    resn: string;
    x: number;
    y: number;
    z: number;
    hetflag: boolean;
  }

  export interface GLViewer {
    addModel(data: string, format: string): GLModel;
    removeAllModels(): void;
    setStyle(sel: AtomSelectionSpec, style: AtomStyleSpec): void;
    addStyle(sel: AtomSelectionSpec, style: AtomStyleSpec): void;
    setBackgroundColor(color: string, alpha?: number): void;
    zoomTo(sel?: AtomSelectionSpec, animationDuration?: number): void;
    zoom(factor: number, animationDuration?: number): void;
    center(sel?: AtomSelectionSpec, animationDuration?: number): void;
    render(): void;
    resize(): void;
    clear(): void;
    spin(axis?: string | boolean, speed?: number): void;
    addLabel(text: string, options: LabelSpec, sel?: AtomSelectionSpec): void;
    removeAllLabels(): void;
    setClickable(
      sel: AtomSelectionSpec,
      clickable: boolean,
      callback?: (atom: Atom, viewer: GLViewer, event: MouseEvent) => void
    ): void;
    setHoverable(
      sel: AtomSelectionSpec,
      hoverable: boolean,
      hoverCallback?: (atom: Atom, viewer: GLViewer, event: MouseEvent) => void,
      unhoverCallback?: (atom: Atom, viewer: GLViewer, event: MouseEvent) => void
    ): void;
    getModel(modelId?: number): GLModel;
    pngURI(): string;
  }

  export function createViewer(
    element: HTMLElement | null,
    config?: ViewerSpec
  ): GLViewer;

  const $3Dmol: {
    createViewer: typeof createViewer;
  };

  export default $3Dmol;
}
