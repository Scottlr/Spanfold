# Public documentation site

The public site is intentionally static and dependency-free. Shared navigation,
language selection, mobile behavior, and syntax highlighting live in the site
scripts and styles rather than a generated framework.

Public discovery starts at [the site overview](index.html). The
[machine-readable documentation index](llms.txt) lists the main guides, language
journeys, API references, and package entry points.

## Adding a page

Start from this shell and keep the fallback navigation small. `site-shell.js`
replaces it with the canonical task-oriented navigation, marks the current
page, installs the skip link, and connects the mobile menu.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="description" content="Describe the user question answered by this page.">
  <title>Page title - Spanfold</title>
  <link rel="icon" href="assets/brand/spanfold-icon.svg" type="image/svg+xml">
  <link rel="stylesheet" href="styles.css">
</head>
<body data-language="csharp">
  <header class="site-header">
    <a class="brand" href="index.html" aria-label="Spanfold home"><img src="assets/brand/spanfold-logo.svg" alt="Spanfold"></a>
    <button class="nav-toggle" aria-label="Toggle navigation" aria-expanded="false">
      <svg width="20" height="20" viewBox="0 0 20 20" fill="none" aria-hidden="true">
        <rect y="3" width="20" height="2" rx="1" fill="currentColor"/>
        <rect y="9" width="20" height="2" rx="1" fill="currentColor"/>
        <rect y="15" width="20" height="2" rx="1" fill="currentColor"/>
      </svg>
    </button>
    <nav class="nav" aria-label="Primary navigation">
      <a href="index.html">Overview</a>
      <a href="get-started.html">Get started</a>
      <a href="concepts.html">Concepts</a>
      <a href="api.html">API</a>
    </nav>
  </header>

  <main>
    <!-- Shared explanation goes here. Use paired language panels only where APIs differ. -->
    <div class="language-panel" data-language="csharp">C# example</div>
    <div class="language-panel" data-language="rust">Rust example</div>
  </main>

  <footer class="site-footer"><span>Spanfold</span></footer>
  <script src="site-shell.js"></script>
  <script src="language.js"></script>
</body>
</html>
```

Register new primary destinations in `site-shell.js`. Detail pages should be
linked from their overview or guide rather than adding every page to the main
navigation. Shared conceptual prose remains visible in both language modes;
use language panels for code, commands, and genuinely language-specific notes.
Pages without paired panels or dedicated C#/Rust routes are shared-neutral, so
the shell does not render a language control on them.

## Dedicated language routes

When a guide has complete language-specific journeys, give each journey its own
URL and connect the header switcher to its counterpart:

```html
<body
  data-language="csharp"
  data-language-route="csharp"
  data-csharp-href="get-started-csharp.html"
  data-rust-href="get-started-rust.html">
```

`data-language-route` makes the URL authoritative when the page opens. The two
route attributes let `language.js` navigate between equivalent C# and Rust
pages while preserving the selected language elsewhere in the site. A neutral
chooser can omit `data-language-route` and keep both route attributes so either
header language control opens the corresponding journey.

The same metadata owns other paired routes, including the API references:

```html
<body
  data-language="rust"
  data-language-route="rust"
  data-csharp-href="api-csharp.html"
  data-rust-href="api-rust.html">
```

Register both dedicated routes against the same parent key in `site-shell.js`.
The primary navigation then marks the API parent with `aria-current="location"`,
while the page's language route selector marks the exact page.
