(function () {
  function normalizeLanguage(language) {
    switch (language) {
      case "csharp":
      case "cs":
        return "csharp";
      case "powershell":
      case "ps":
      case "ps1":
        return "powershell";
      case "xml":
      case "html":
        return "xml";
      case "bash":
      case "shell":
      case "sh":
      default:
        return language || "plaintext";
    }
  }

  function panelLanguage(code) {
    const panel = code.closest("[data-language]");
    return panel ? normalizeLanguage(panel.dataset.language) : "";
  }

  function detectLanguage(code) {
    const existing = Array.from(code.classList)
      .find((className) => className.startsWith("language-"));

    if (existing) {
      return normalizeLanguage(existing.replace("language-", ""));
    }

    const text = code.textContent.trim();

    if (!text) {
      return "plaintext";
    }

    if (/^<|\bPackageReference\b/.test(text)) {
      return "xml";
    }

    if (/^Install-Package\b/im.test(text)) {
      return "powershell";
    }

    if (/^(cd|dotnet|cargo)\b/im.test(text)) {
      return "bash";
    }

    if (/^(closed windows|overlap rows|provider-|a-only|overlap )/im.test(text)) {
      return "plaintext";
    }

    const language = panelLanguage(code);
    if (language) {
      return language;
    }

    if (/\b(using|public|sealed|record|foreach|var)\b|=>/.test(text)) {
      return "csharp";
    }

    return "plaintext";
  }

  function applyHighlighting() {
    document.querySelectorAll("pre code").forEach((code) => {
      const language = detectLanguage(code);
      code.classList.add("language-" + language);
      code.parentElement.dataset.language = language;

      if (window.hljs && language !== "plaintext") {
        window.hljs.highlightElement(code);
      }
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", applyHighlighting);
  } else {
    applyHighlighting();
  }
})();
