const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

function setupAnchorNavigation() {
  const navLinks = [...document.querySelectorAll(".nav-links a")];
  const selectableLinks = navLinks.filter((link) => {
    const href = link.getAttribute("href") || "";
    return href.startsWith("#") && document.querySelector(href);
  });

  function setActiveLink(hash) {
    const activeHash = hash || "#product";
    navLinks.forEach((link) => link.classList.remove("active"));
    selectableLinks
      .find((link) => link.getAttribute("href") === activeHash)
      ?.classList.add("active");
  }

  selectableLinks.forEach((link) => {
    link.addEventListener("click", () => setActiveLink(link.getAttribute("href")));
  });

  window.addEventListener("hashchange", () => setActiveLink(window.location.hash));
  setActiveLink(window.location.hash);
}

function initForgeMotion() {
  setupAnchorNavigation();

  if (!window.gsap) {
    document.documentElement.dataset.motion = "unavailable";
    return;
  }

  const { gsap } = window;
  const hasScrollTrigger = Boolean(window.ScrollTrigger);
  document.documentElement.dataset.motion = "on";
  document.documentElement.dataset.gsapVersion = gsap.version || "unknown";

  if (hasScrollTrigger) {
    gsap.registerPlugin(window.ScrollTrigger);
  }

  gsap.defaults({
    duration: 0.72,
    ease: "power3.out",
  });

  if (prefersReducedMotion.matches) {
    document.documentElement.dataset.motion = "reduced";
    gsap.set(
      [
        ".site-header",
        ".hero-copy > *",
        ".product-window",
        ".stream-block",
        ".file-strip",
        ".approval-card",
        ".section-heading",
        ".feature-panel",
      ],
      { clearProps: "all" },
    );
    return;
  }

  const intro = gsap.timeline({ defaults: { ease: "power3.out" } });

  intro
    .from(".site-header", {
      y: -18,
      autoAlpha: 0,
      duration: 0.48,
    })
    .from(
      ".hero-copy > *",
      {
        y: 26,
        autoAlpha: 0,
        duration: 0.68,
        stagger: 0.08,
      },
      "-=0.12",
    )
    .from(
      ".product-window",
      {
        x: 34,
        y: 26,
        scale: 0.985,
        autoAlpha: 0,
        duration: 0.86,
      },
      "-=0.58",
    )
    .from(
      ".session-row",
      {
        x: -14,
        autoAlpha: 0,
        duration: 0.42,
        stagger: 0.07,
      },
      "-=0.38",
    )
    .from(
      ".stream-block, .file-strip",
      {
        y: 18,
        autoAlpha: 0,
        duration: 0.52,
        stagger: 0.09,
      },
      "-=0.3",
    )
    .from(
      ".approval-card",
      {
        y: 22,
        scale: 0.975,
        autoAlpha: 0,
        duration: 0.62,
      },
      "-=0.22",
    );

  gsap.to(".session-row.active .session-dot", {
    boxShadow:
      "0 0 0 2px rgba(88, 225, 129, 0.12) inset, 0 0 22px rgba(88, 225, 129, 0.32)",
    repeat: -1,
    yoyo: true,
    duration: 1.45,
    ease: "sine.inOut",
  });

  gsap.to(".approval-card", {
    boxShadow:
      "0 18px 50px rgba(0, 0, 0, 0.34), 0 0 28px rgba(216, 173, 80, 0.16)",
    repeat: -1,
    yoyo: true,
    duration: 2.4,
    ease: "sine.inOut",
  });

  gsap.to(".progress span", {
    scaleX: 0.82,
    transformOrigin: "left center",
    repeat: -1,
    yoyo: true,
    duration: 1.8,
    ease: "sine.inOut",
  });

  document.querySelectorAll(".button, .header-cta").forEach((element) => {
    element.addEventListener("pointerenter", () => {
      gsap.to(element, { y: -2, duration: 0.22, overwrite: "auto" });
    });
    element.addEventListener("pointerleave", () => {
      gsap.to(element, { y: 0, duration: 0.28, overwrite: "auto" });
    });
  });

  if (!hasScrollTrigger) return;

  gsap.matchMedia().add("(min-width: 921px)", () => {
    gsap.to(".product-window", {
      y: -18,
      ease: "none",
      scrollTrigger: {
        trigger: ".hero",
        start: "top top",
        end: "bottom top",
        scrub: 0.8,
      },
    });
  });

  gsap.from(".section-heading", {
    y: 34,
    autoAlpha: 0,
    duration: 0.7,
    scrollTrigger: {
      trigger: ".visibility-section",
      start: "top 72%",
      once: true,
    },
  });

  gsap.from(".feature-panel", {
    y: 32,
    autoAlpha: 0,
    duration: 0.72,
    stagger: 0.12,
    scrollTrigger: {
      trigger: ".feature-grid",
      start: "top 78%",
      once: true,
    },
  });

  gsap.from(".trace-node, .trace-card, .tool-row", {
    y: 18,
    autoAlpha: 0,
    duration: 0.42,
    stagger: 0.055,
    scrollTrigger: {
      trigger: ".feature-grid",
      start: "top 58%",
      once: true,
    },
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initForgeMotion);
} else {
  initForgeMotion();
}
