(function () {
  const measurementId = "G-RDNDN2RW33";
  const storageKey = "spanfold-language";
  const supported = new Set(["csharp", "rust"]);

  function installGoogleAnalytics() {
    if (window.gtag) {
      return;
    }
    const script = document.createElement("script");
    script.async = true;
    script.src = `https://www.googletagmanager.com/gtag/js?id=${measurementId}`;
    document.head.appendChild(script);

    window.dataLayer = window.dataLayer || [];
    window.gtag = function gtag() {
      window.dataLayer.push(arguments);
    };
    window.gtag("js", new Date());
    window.gtag("config", measurementId);
  }

  installGoogleAnalytics();

  function resolveLanguage(value) {
    return supported.has(value) ? value : "csharp";
  }

  function setLanguage(language) {
    const resolved = resolveLanguage(language);
    document.body.dataset.language = resolved;
    document.querySelectorAll("[data-language-toggle]").forEach((button) => {
      button.setAttribute("aria-pressed", String(button.dataset.languageToggle === resolved));
    });
    localStorage.setItem(storageKey, resolved);
  }

  document.addEventListener("DOMContentLoaded", () => {
    setLanguage(localStorage.getItem(storageKey) || document.body.dataset.language);

    document.querySelectorAll("[data-language-toggle]").forEach((button) => {
      button.addEventListener("click", () => {
        setLanguage(button.dataset.languageToggle);
      });
    });

    const navToggle = document.querySelector(".nav-toggle");
    const siteHeader = document.querySelector(".site-header");
    if (navToggle && siteHeader) {
      function closeNav() {
        siteHeader.classList.remove("nav-open");
        navToggle.setAttribute("aria-expanded", "false");
      }

      navToggle.addEventListener("click", () => {
        const isOpen = siteHeader.classList.toggle("nav-open");
        navToggle.setAttribute("aria-expanded", String(isOpen));
      });
      document.querySelectorAll(".nav a").forEach((link) => {
        link.addEventListener("click", closeNav);
      });
      document.addEventListener("click", (event) => {
        if (!siteHeader.classList.contains("nav-open")) {
          return;
        }
        if (!siteHeader.contains(event.target)) {
          closeNav();
        }
      });
      document.addEventListener("keydown", (event) => {
        if (event.key === "Escape") {
          closeNav();
        }
      });
    }
  });
})();
