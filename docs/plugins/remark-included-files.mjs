// Some pages include a file of the repository instead of copying it (plan D68): the changelog
// page includes CHANGELOG.md and the working-agreements page includes AGENTS.md. Their own
// "# …" title would repeat the page title, so it is removed when they are rendered for the site.

import path from 'node:path';

/** @param {{ files: string[] }} options absolute paths of the included files */
export default function remarkIncludedFiles({ files }) {
  const included = new Set(files.map((f) => path.resolve(f)));
  return (tree, file) => {
    if (!file.history[0] || !included.has(path.resolve(file.history[0]))) return;
    const index = tree.children.findIndex((n) => n.type === 'heading' && n.depth === 1);
    if (index !== -1) tree.children.splice(index, 1);
  };
}
