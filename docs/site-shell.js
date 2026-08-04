(function () {
  const navigation = [
    {
      title: "Start",
      links: [
        { key: "overview", label: "Overview", href: "index.html" },
        { key: "get-started", label: "Get started", href: "get-started.html" },
      ],
    },
    {
      title: "Learn",
      links: [
        { key: "concepts", label: "Core concepts", href: "concepts.html" },
        { key: "use-cases", label: "Use cases", href: "use-cases.html" },
      ],
    },
    {
      title: "Guides",
      links: [
        { key: "query-history", label: "Query history", href: "concepts-querying-history.html" },
        { key: "compare-histories", label: "Compare histories", href: "concepts-comparing-histories.html" },
        { key: "live-finality", label: "Live finality", href: "concepts-live-finality.html" },
        { key: "advanced", label: "Advanced analytics", href: "concepts-advanced-analytics.html" },
      ],
    },
    {
      title: "Tools",
      links: [
        { key: "visual-auditing", label: "Visual auditing", href: "visualiser.html" },
        { key: "agentic-auditing", label: "Agentic auditing", href: "llm-context.html" },
      ],
    },
    {
      title: "Reference",
      links: [
        { key: "api", label: "API reference", href: "api.html" },
        { label: "GitHub", href: "https://github.com/Scottlr/spanfold" },
      ],
    },
  ];

  const pageKeys = new Map([
    ["index.html", "overview"],
    ["", "overview"],
    ["get-started.html", "get-started"],
    ["concepts.html", "concepts"],
    ["use-cases.html", "use-cases"],
    ["concepts-querying-history.html", "query-history"],
    ["concepts-comparing-histories.html", "compare-histories"],
    ["concepts-live-finality.html", "live-finality"],
    ["concepts-advanced-analytics.html", "advanced"],
    ["visualiser.html", "visual-auditing"],
    ["llm-context.html", "agentic-auditing"],
    ["api.html", "api"],
  ]);

  function currentPageKey() {
    const fileName = window.location.pathname.split("/").pop() || "";
    if (pageKeys.has(fileName)) {
      return pageKeys.get(fileName);
    }
    return fileName.startsWith("concepts-") ? "concepts" : null;
  }

  function buildNavigation(nav) {
    const activeKey = currentPageKey();
    const fragment = document.createDocumentFragment();

    navigation.forEach((section) => {
      const container = document.createElement("div");
      container.className = "nav-section";

      const title = document.createElement("span");
      title.className = "nav-title";
      title.textContent = section.title;
      container.appendChild(title);

      section.links.forEach((item) => {
        const link = document.createElement("a");
        link.href = item.href;
        link.textContent = item.label;
        if (item.key === activeKey) {
          link.className = "active";
          link.setAttribute("aria-current", "page");
        }
        container.appendChild(link);
      });

      fragment.appendChild(container);
    });

    const language = document.createElement("div");
    language.className = "nav-section nav-language";
    language.innerHTML = `
      <span class="nav-title">Language</span>
      <div class="language-switcher" aria-label="Documentation language">
        <button type="button" data-language-toggle="csharp" aria-pressed="true">C#</button>
        <button type="button" data-language-toggle="rust" aria-pressed="false">Rust</button>
      </div>`;
    fragment.appendChild(language);

    nav.replaceChildren(fragment);
  }

  function installPageShell() {
    const main = document.querySelector("main");
    if (main && !main.id) {
      main.id = "main-content";
    }

    if (main && !document.querySelector(".skip-link")) {
      const skipLink = document.createElement("a");
      skipLink.className = "skip-link";
      skipLink.href = "#main-content";
      skipLink.textContent = "Skip to content";
      document.body.prepend(skipLink);
    }

    const nav = document.querySelector(".nav");
    if (nav) {
      nav.id = "site-navigation";
      buildNavigation(nav);
    }

    const toggle = document.querySelector(".nav-toggle");
    if (toggle) {
      toggle.setAttribute("aria-controls", "site-navigation");
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", installPageShell);
  } else {
    installPageShell();
  }
})();
