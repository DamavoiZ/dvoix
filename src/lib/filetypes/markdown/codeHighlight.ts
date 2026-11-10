// Syntax highlighting of fenced code blocks in the reading view. The editor
// highlights them through CodeMirror's nested languages; here the same
// parsers and the same `tok-*` classes (colors in `src/app.css`) are used, so
// a block looks the same in both views.
import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { classHighlighter, highlightCode } from "@lezer/highlight";

/** Builds the highlighted nodes of one code block. */
async function highlight(code: HTMLElement, name: string): Promise<DocumentFragment | null> {
  const description = LanguageDescription.matchLanguageName(languages, name, true);
  if (!description) return null;
  // A language is loaded on first use, like in the editor.
  const support = await description.load();
  const text = code.textContent ?? "";
  const fragment = document.createDocumentFragment();
  highlightCode(
    text,
    support.language.parser.parse(text),
    classHighlighter,
    (token, classes) => {
      if (!classes) {
        fragment.append(token);
        return;
      }
      const span = document.createElement("span");
      span.className = classes;
      span.textContent = token;
      fragment.append(span);
    },
    () => fragment.append("\n"),
  );
  return fragment;
}

/**
 * Highlights every `<pre><code class="language-…">` below `root` and puts its
 * language on the `<pre>` (`data-language`), where the CSS shows it as a
 * label. Blocks of an unknown language stay plain text.
 */
export async function highlightCodeBlocks(root: HTMLElement): Promise<void> {
  const blocks = root.querySelectorAll<HTMLElement>('pre > code[class*="language-"]');
  await Promise.all(
    [...blocks].map(async (code) => {
      const name = [...code.classList].find((c) => c.startsWith("language-"))?.slice("language-".length);
      if (!name) return;
      code.parentElement?.setAttribute("data-language", name);
      try {
        const fragment = await highlight(code, name);
        // The note may have been rendered again while the language loaded.
        if (fragment && code.isConnected) code.replaceChildren(fragment);
      } catch {
        // A language that fails to load leaves the block as plain text.
      }
    }),
  );
}
