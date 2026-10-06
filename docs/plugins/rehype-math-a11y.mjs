// Accessible MathJax output (WCAG 2.2 SC 1.1.1 and 2.1.1).
//
// rehype-mathjax renders TeX (from remark-math) as SVG with `role="img"` but no text
// alternative, and renders `\eqref` as a link inside the SVG. These two small plugins wrap it:
//
// * `rehypeMathPrepare` runs before rehype-mathjax. It converts each formula to MathML with the
//   same MathJax TeX input (AMS numbering, so labels and numbers agree) and places the MathML
//   next to the formula, visually hidden, for screen readers (the approach of MathJax's own
//   "assistive MathML"). A formula that is only `\eqref{label}` becomes a plain HTML link to the
//   equation, so it is keyboard- and screen-reader-accessible. Display formulas are wrapped in a
//   focusable block that scrolls horizontally on narrow screens instead of widening the page.
//   TeX errors and references to unknown labels fail the build.
// * `rehypeMathFinish` runs after rehype-mathjax and hides the SVG drawings from assistive
//   technology (the MathML speaks for them).

import { fromHtml } from 'hast-util-from-html';
import { toText } from 'hast-util-to-text';
import { h } from 'hastscript';
import { liteAdaptor } from 'mathjax-full/js/adaptors/liteAdaptor.js';
import { STATE } from 'mathjax-full/js/core/MathItem.js';
import { SerializedMmlVisitor } from 'mathjax-full/js/core/MmlTree/SerializedMmlVisitor.js';
import { RegisterHTMLHandler } from 'mathjax-full/js/handlers/html.js';
import { TeX } from 'mathjax-full/js/input/tex.js';
import { AllPackages } from 'mathjax-full/js/input/tex/AllPackages.js';
import { mathjax } from 'mathjax-full/js/mathjax.js';
import { visit } from 'unist-util-visit';
import { SKIP, visitParents } from 'unist-util-visit-parents';

/** A `$\eqref{…}$` or `$\ref{…}$` formula on its own. */
const REFERENCE = /^\s*\\(?:eq)?ref\s*\{[^}]*\}\s*$/;

/** Throws TeX errors instead of rendering them, so they fail the build. */
function throwTexError(_jax, error) {
  throw new Error(`TeX error: ${error.message}`);
}

/**
 * A TeX → MathML converter with its own equation numbering (one per page, like rehype-mathjax).
 *
 * @param {Record<string, unknown>} texOptions
 */
function createConverter(texOptions) {
  const handler = RegisterHTMLHandler(liteAdaptor());
  // bussproofs needs an output jax to measure boxes; MathML needs none (as in MathJax's tex2mml).
  const packages = AllPackages.filter((name) => name !== 'bussproofs');
  const tex = new TeX({ packages, ...texOptions, formatError: throwTexError });
  const document = mathjax.document('', { InputJax: tex });
  const visitor = new SerializedMmlVisitor();
  return {
    toMathml(text, display) {
      return visitor.visitTree(document.convert(text, { display, end: STATE.CONVERT }));
    },
    done() {
      mathjax.handlers.unregister(handler);
    },
  };
}

/** Classes that remark-math (and fenced ```math blocks) put on formulas, as rehype-mathjax reads them. */
function mathKind(element) {
  const classes = Array.isArray(element.properties?.className) ? element.properties.className : [];
  const languageMath = classes.includes('language-math');
  const display = classes.includes('math-display');
  const inline = classes.includes('math-inline');
  return languageMath || display || inline ? { languageMath, display } : null;
}

/**
 * Runs before rehype-mathjax.
 *
 * @param {{ tex?: Record<string, unknown> }} [options] the TeX options given to rehype-mathjax
 */
export function rehypeMathPrepare(options = {}) {
  return (tree, file) => {
    let converter = null;
    try {
      visitParents(tree, 'element', (element, parents) => {
        const kind = mathKind(element);
        if (!kind) return undefined;
        let parent = parents.at(-1);
        let scope = element;
        let display = kind.display;
        if (element.tagName === 'code' && kind.languageMath && parent?.tagName === 'pre') {
          scope = parent;
          parent = parents.at(-2);
          display = true;
        }
        if (!parent) return undefined;
        converter ??= createConverter(options.tex ?? {});
        const text = toText(scope, { whitespace: 'pre' });
        let mathml;
        try {
          mathml = converter.toMathml(text, display);
        } catch (error) {
          file.fail(`${error.message} in formula: ${text.trim()}`, scope);
        }
        const index = parent.children.indexOf(scope);
        if (REFERENCE.test(text)) {
          const href = /href="([^"]*)"/.exec(mathml)?.[1];
          const label = /<mtext>([^<]*)<\/mtext>/.exec(mathml)?.[1];
          if (!href || !label || label.includes('?')) {
            file.fail(`unknown equation label in ${text.trim()} (labels must precede references)`, scope);
          }
          parent.children.splice(index, 1, h('a', { href: decodeURIComponent(href), className: ['dima-eqref'] }, label));
          return SKIP;
        }
        const speech = h('span', { className: ['sr-only'] }, fromHtml(mathml, { fragment: true }).children);
        // The drawing carries the equation-number anchors; ids must stay unique.
        visit(speech, 'element', (node) => {
          delete node.properties.id;
        });
        const wrapper = display
          ? h('div', { className: ['dima-math', 'dima-math-block'], tabIndex: 0 }, [scope, speech])
          : h('span', { className: ['dima-math'] }, [scope, speech]);
        parent.children.splice(index, 1, wrapper);
        return SKIP;
      });
    } finally {
      converter?.done();
    }
  };
}

/**
 * Runs after rehype-mathjax: hides the SVG drawings (and any links inside them) from assistive
 * technology, and drops the per-page MathJax style sheet (MDX would not render it as CSS; the same
 * rules are in src/styles/custom.css).
 */
export function rehypeMathFinish() {
  return (tree) => {
    visit(tree, 'element', (element, index, parent) => {
      if (element.tagName === 'style' && parent && index !== undefined && toText(element).includes('mjx-container[jax="SVG"]')) {
        parent.children.splice(index, 1);
        return [SKIP, index];
      }
      if (element.tagName !== 'mjx-container') return undefined;
      for (const child of element.children) {
        if (child.type === 'element' && child.tagName === 'svg') {
          child.properties.ariaHidden = 'true';
          visit(child, 'element', (node) => {
            if (node.tagName === 'a') delete node.properties.href;
          });
        }
      }
      return SKIP;
    });
  };
}
