import { useMemo } from "react";

const PREVIEW_CSS = `
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  line-height: 1.6;
  color: #1a1a1a;
  max-width: 48rem;
  margin: 0 auto;
  padding: 1.5rem 2rem;
  font-size: 15px;
}
h1, h2, h3, h4, h5, h6 {
  margin-top: 1.5em;
  margin-bottom: 0.5em;
  line-height: 1.3;
}
h1 { font-size: 1.8em; border-bottom: 1px solid #e5e5e5; padding-bottom: 0.3em; }
h2 { font-size: 1.4em; border-bottom: 1px solid #e5e5e5; padding-bottom: 0.2em; }
h3 { font-size: 1.15em; }
p { margin: 0.8em 0; }
a { color: #2563eb; text-decoration: none; }
a:hover { text-decoration: underline; }
code {
  font-family: "JetBrains Mono", "Fira Code", ui-monospace, monospace;
  font-size: 0.88em;
  background: #f3f4f6;
  padding: 0.15em 0.35em;
  border-radius: 4px;
}
pre {
  background: #f8f9fa;
  border: 1px solid #e5e7eb;
  border-radius: 6px;
  padding: 0.9em 1.1em;
  overflow-x: auto;
  line-height: 1.5;
}
pre code { background: none; padding: 0; font-size: 0.85em; }
blockquote {
  border-left: 3px solid #d1d5db;
  margin: 0.8em 0;
  padding: 0.4em 1em;
  color: #4b5563;
}
table {
  border-collapse: collapse;
  width: 100%;
  margin: 1em 0;
  font-size: 0.92em;
}
th, td {
  border: 1px solid #d1d5db;
  padding: 0.5em 0.75em;
  text-align: left;
}
th { background: #f3f4f6; font-weight: 600; }
tr:nth-child(even) { background: #f9fafb; }
img { max-width: 100%; height: auto; }
hr { border: none; border-top: 1px solid #e5e7eb; margin: 1.5em 0; }
ul, ol { padding-left: 1.5em; margin: 0.5em 0; }
li { margin: 0.25em 0; }
.math-display {
  overflow-x: auto;
  margin: 1em 0;
  text-align: center;
}
`;

// Render .math elements via KaTeX API instead of auto-render's delimiter scan.
// The HTML writer already parses $..$ / $$...$$ into <span class="math"> elements,
// so there are no raw delimiters left for auto-render to find.
const MATH_SCRIPT = `
<script>
document.addEventListener("DOMContentLoaded", function() {
  if (typeof katex === "undefined") return;
  document.querySelectorAll(".math").forEach(function(el) {
    try {
      katex.render(el.textContent, el, {
        displayMode: el.classList.contains("math-display"),
        throwOnError: false
      });
    } catch (e) {}
  });
});
</script>`;

function injectPreviewAssets(html: string): string {
  const styleTag = `<style>${PREVIEW_CSS}</style>`;
  if (html.includes("</head>")) {
    // Remove the auto-render script (useless since delimiters are already parsed)
    // and inject our direct-render script + styles instead
    return html
      .replace(/<script defer src="[^"]*auto-render[^"]*"[^>]*><\/script>/g, "")
      .replace("</head>", `${styleTag}\n${MATH_SCRIPT}\n</head>`);
  }
  return styleTag + MATH_SCRIPT + html;
}

interface HtmlPreviewProps {
  html: string | null;
}

export function HtmlPreview({ html }: HtmlPreviewProps) {
  const styledHtml = useMemo(
    () => (html ? injectPreviewAssets(html) : null),
    [html],
  );

  if (!styledHtml) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-zinc-600">
        No preview available
      </div>
    );
  }

  return (
    <iframe
      srcDoc={styledHtml}
      sandbox="allow-scripts allow-same-origin"
      title="HTML Preview"
      className="h-full w-full border-0 bg-white"
    />
  );
}
