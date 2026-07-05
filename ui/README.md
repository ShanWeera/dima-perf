# DiMA Desktop - Frontend

Cross-platform desktop GUI for the DiMA (Diversity Motif Analyser) bioinformatics tool.

## Tech Stack

- **Framework**: React 18 + TypeScript
- **Backend**: Tauri 2 (Rust)
- **State Management**: Zustand 5
- **Styling**: Tailwind CSS 3 + shadcn/ui
- **Charts**: ECharts (via echarts-for-react)
- **3D Visualization**: 3Dmol.js
- **Dashboard**: react-grid-layout

## Development Setup

### Prerequisites

- Node.js >= 22
- Rust toolchain (rustup)
- Tauri CLI: `cargo install tauri-cli`
- System dependencies for Tauri (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

### Getting Started

```bash
# Install frontend dependencies
cd ui
npm install

# Run in development mode (from project root)
cargo tauri dev
```

### Available Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Start Vite dev server (used by Tauri) |
| `npm run build` | TypeScript check + Vite production build |
| `npm run lint` | Run ESLint |
| `npm run test` | Run Vitest |

### Building for Production

```bash
# From project root
cargo tauri build
```

## Architecture

```
ui/
├── src/
│   ├── components/     # React components
│   │   ├── charts/     # ECharts + PDB viewer
│   │   ├── dashboard/  # Drag-and-drop dashboard grid
│   │   ├── dialogs/    # Modal dialogs
│   │   ├── export/     # Export functionality
│   │   ├── features/   # UniProt feature tracks
│   │   ├── layout/     # App shell (sidebar, main content)
│   │   ├── ui/         # shadcn/ui primitives
│   │   ├── views/      # Top-level views
│   │   └── wizard/     # Analysis wizard steps
│   ├── hooks/          # Custom React hooks
│   ├── lib/            # Utilities, types, Tauri API wrappers
│   └── stores/         # Zustand state management
├── index.html
├── tailwind.config.js
├── tsconfig.json
└── vite.config.ts
```

### Key Design Decisions

- **Single IPC Layer**: All Tauri commands are wrapped in `lib/tauri.ts` for type safety
- **Generation IDs**: Async operations use generation counters to prevent stale responses
- **Debounced Persistence**: Layout, annotations, and filters save with debouncing
- **Code Splitting**: Heavy dependencies (ECharts, 3Dmol) are lazy-loaded
- **Theme Support**: Light/dark mode via CSS variables + Zustand store
