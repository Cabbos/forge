import { useMemo, useState } from "react";
import {
  ArrowDownToLine,
  Check,
  ChevronDown,
  Command,
  Database,
  FileText,
  FolderOpen,
  GitBranch,
  KeyRound,
  Layers3,
  LockKeyhole,
  Menu,
  Play,
  Search,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  X,
} from "lucide-react";

const screenshots = {
  workbench: {
    label: "Workbench",
    src: "/assets/forge-active-workspace.png",
    alt: "Forge active workspace showing a project-bound composer and local shell context",
    title: "The workbench",
    detail: "Project, prompt, shell, evidence, and handoff stay in view.",
  },
  archive: {
    label: "Project archive",
    src: "/assets/forge-project-archive.png",
    alt: "Forge project archive with delivery state and project records",
    title: "The archive",
    detail: "Every project keeps its thread, decisions, checkpoints, and delivery state.",
  },
};

const highlights = [
  {
    icon: FolderOpen,
    title: "Choose a project first.",
    copy: "Forge starts where real coding starts: inside a local folder with a visible boundary.",
  },
  {
    icon: TerminalSquare,
    title: "Watch the work happen.",
    copy: "Shell output, file reads, diffs, confirmations, and failures become one readable event stream.",
  },
  {
    icon: Layers3,
    title: "Come back without starting over.",
    copy: "Project archives keep the current task, useful context, and delivery state ready for the next session.",
  },
];

const workflow = [
  ["Select", "Pick a local project or resume a recent one.", FolderOpen],
  ["Ask", "Describe the change, bug, feature, or investigation.", Sparkles],
  ["Confirm", "Approve risky writes and commands before they run.", ShieldCheck],
  ["Deliver", "Review the output, checkpoints, and next action.", Check],
];

const safety = [
  {
    icon: LockKeyhole,
    title: "Keys stay local",
    copy: "Provider setup is surfaced in the desktop settings flow, without hiding prerequisites inside a chat transcript.",
  },
  {
    icon: GitBranch,
    title: "Workspace is explicit",
    copy: "The active folder appears in the composer, shell surface, archive, and project switcher before action starts.",
  },
  {
    icon: FileText,
    title: "Evidence is structured",
    copy: "Routine progress stays calm. Confirmations, failed checks, diffs, and recovery prompts get dedicated structure.",
  },
];

const faqItems = [
  {
    question: "Forge 是 IDE 吗？",
    answer:
      "不是。Forge 不替代 IDE、Git 或终端，它把 coding agent 的执行过程放进一个可见、可确认、可恢复的桌面工作台。",
  },
  {
    question: "为什么这版更像 Apple 产品页？",
    answer:
      "Forge 本身是桌面工具，真实界面已经有 Mac workbench 的气质。Apple 风更适合把产品界面放大讲清楚，少做泛 AI 官网的装饰。",
  },
  {
    question: "可以连接哪些模型？",
    answer:
      "真实产品里已有 DeepSeek、Anthropic、OpenAI、OpenRouter 等 Provider 入口。这个官网原型保留产品信息，但不模拟密钥配置。",
  },
];

const commandRows = [
  ["New task", "⌘N", Sparkles],
  ["Open project archive", "⌘I", Layers3],
  ["Switch project", "⌘O", FolderOpen],
  ["Provider settings", "⌘,", KeyRound],
  ["Local data", "local", Database],
];

export function App() {
  const [galleryMode, setGalleryMode] = useState("workbench");
  const [activeHighlight, setActiveHighlight] = useState(0);
  const [faqOpen, setFaqOpen] = useState(0);
  const [menuOpen, setMenuOpen] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [toast, setToast] = useState("");

  const activeShot = useMemo(() => screenshots[galleryMode], [galleryMode]);
  const highlighted = highlights[activeHighlight];
  const HighlightIcon = highlighted.icon;

  function closeMenu() {
    setMenuOpen(false);
  }

  function handleDownload() {
    setToast("Mac 版下载入口已就绪。这个原型先展示按钮状态。");
    window.setTimeout(() => setToast(""), 3200);
  }

  return (
    <main className="site-shell">
      <header className="site-nav">
        <a className="brand" href="#top" aria-label="Forge home" onClick={closeMenu}>
          <img src="/assets/forge-mark.svg" alt="" />
          <span>Forge</span>
        </a>

        <nav className={menuOpen ? "nav-links nav-links-open" : "nav-links"} aria-label="Primary navigation">
          <a href="#highlights" onClick={closeMenu}>
            Highlights
          </a>
          <a href="#closer-look" onClick={closeMenu}>
            Closer look
          </a>
          <a href="#local" onClick={closeMenu}>
            Local
          </a>
          <a href="#faq" onClick={closeMenu}>
            FAQ
          </a>
        </nav>

        <div className="nav-actions">
          <button className="nav-command" type="button" onClick={() => setCommandOpen(true)}>
            <Command size={15} />
            <span>Command</span>
          </button>
          <button className="download-button nav-download" type="button" onClick={handleDownload}>
            <ArrowDownToLine size={15} />
            <span>Download</span>
          </button>
          <button
            className="icon-button menu-button"
            type="button"
            aria-label={menuOpen ? "Close menu" : "Open menu"}
            onClick={() => setMenuOpen((value) => !value)}
          >
            {menuOpen ? <X size={18} /> : <Menu size={18} />}
          </button>
        </div>
      </header>

      <section id="top" className="hero-section">
        <div className="hero-copy">
          <h1>Forge</h1>
          <p className="hero-tagline">Agent work, on your Mac.</p>
          <p className="hero-lede">
            A local-first workbench for coding agents. Choose a real project, ask for the next move,
            and keep context, execution evidence, approvals, and delivery state together.
          </p>
          <div className="hero-actions" aria-label="Primary actions">
            <button className="download-button hero-download" type="button" onClick={handleDownload}>
              <ArrowDownToLine size={18} />
              <span>Download for Mac</span>
            </button>
            <a className="text-link" href="#closer-look">
              <Play size={16} />
              <span>Take a closer look</span>
            </a>
          </div>
        </div>

        <div className="hero-product" aria-label="Forge product screenshot">
          <div className="window-bar">
            <span className="window-dot red" />
            <span className="window-dot yellow" />
            <span className="window-dot green" />
            <strong>crusted-spinning-lynx-agent</strong>
          </div>
          <img src="/assets/forge-active-workspace.png" alt="Forge desktop app workbench" />
        </div>

        <div className="hero-proof" aria-label="Product proof points">
          <span>Local projects</span>
          <span>Visible shell evidence</span>
          <span>Explicit confirmations</span>
          <span>Recoverable archives</span>
        </div>
      </section>

      <section id="highlights" className="highlights-section">
        <div className="section-title">
          <h2>Get the highlights.</h2>
          <button className="text-link command-link" type="button" onClick={() => setCommandOpen(true)}>
            <Command size={16} />
            <span>Open command palette</span>
          </button>
        </div>

        <div className="highlight-showcase">
          <div className="highlight-copy">
            <HighlightIcon size={26} />
            <h3>{highlighted.title}</h3>
            <p>{highlighted.copy}</p>
            <div className="highlight-controls" role="tablist" aria-label="Forge highlights">
              {highlights.map((item, index) => (
                <button
                  key={item.title}
                  type="button"
                  className={activeHighlight === index ? "active" : ""}
                  role="tab"
                  aria-selected={activeHighlight === index}
                  onClick={() => setActiveHighlight(index)}
                >
                  <span>{String(index + 1).padStart(2, "0")}</span>
                  {item.title}
                </button>
              ))}
            </div>
          </div>
          <div className="highlight-device">
            <img
              src={activeHighlight === 2 ? "/assets/forge-project-archive.png" : "/assets/forge-active-workspace.png"}
              alt={activeHighlight === 2 ? "Forge project archive screenshot" : "Forge workbench screenshot"}
            />
          </div>
        </div>
      </section>

      <section id="closer-look" className="closer-section">
        <div className="section-title centered">
          <h2>Take a closer look.</h2>
          <p>Two real product states, placed front and center.</p>
        </div>

        <div className="gallery-tabs" role="tablist" aria-label="Product gallery">
          {Object.entries(screenshots).map(([key, item]) => (
            <button
              key={key}
              type="button"
              className={galleryMode === key ? "active" : ""}
              role="tab"
              aria-selected={galleryMode === key}
              onClick={() => setGalleryMode(key)}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="gallery-frame">
          <img src={activeShot.src} alt={activeShot.alt} />
        </div>

        <div className="gallery-caption">
          <h3>{activeShot.title}</h3>
          <p>{activeShot.detail}</p>
        </div>
      </section>

      <section id="local" className="local-section">
        <div className="local-copy">
          <h2>Everything begins with a folder.</h2>
          <p>
            Forge keeps the agent attached to the project in front of you, so the work has a place,
            a record, and a clear point of return.
          </p>
        </div>

        <div className="workflow-strip" aria-label="Forge workflow">
          {workflow.map(([title, copy, Icon], index) => (
            <article key={title}>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <Icon size={20} />
              <h3>{title}</h3>
              <p>{copy}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="safety-section">
        <div className="section-title centered light">
          <h2>Local by default. Explicit by design.</h2>
          <p>Forge is calm when things are routine, and clear when the agent needs permission.</p>
        </div>

        <div className="safety-grid">
          {safety.map((item) => (
            <article key={item.title}>
              <item.icon size={22} />
              <h3>{item.title}</h3>
              <p>{item.copy}</p>
            </article>
          ))}
        </div>

        <div className="command-preview">
          <div className="command-input">
            <Search size={17} />
            <span>Search commands and projects</span>
            <kbd>⌘K</kbd>
          </div>
          <div className="command-list">
            {commandRows.slice(0, 4).map(([label, meta, Icon]) => (
              <button key={label} type="button">
                <Icon size={16} />
                <span>{label}</span>
                <em>{meta}</em>
              </button>
            ))}
          </div>
        </div>
      </section>

      <section className="download-section">
        <div>
          <h2>Bring agent work back to the desktop.</h2>
          <p>Forge is early, local-first, and focused on making real project work inspectable.</p>
        </div>
        <button className="download-button" type="button" onClick={handleDownload}>
          <ArrowDownToLine size={18} />
          <span>Download for Mac</span>
        </button>
      </section>

      <section id="faq" className="faq-section">
        <div className="section-title centered">
          <h2>FAQ</h2>
          <p>Short answers for the first public story.</p>
        </div>

        <div className="faq-list">
          {faqItems.map((item, index) => (
            <article key={item.question} className={faqOpen === index ? "faq-item open" : "faq-item"}>
              <button type="button" onClick={() => setFaqOpen(faqOpen === index ? -1 : index)}>
                <span>{item.question}</span>
                <ChevronDown size={18} />
              </button>
              <p>{item.answer}</p>
            </article>
          ))}
        </div>
      </section>

      <footer className="site-footer">
        <a className="brand footer-brand" href="#top">
          <img src="/assets/forge-mark.svg" alt="" />
          <span>Forge</span>
        </a>
        <p>Local agent workbench for real projects.</p>
        <div>
          <a href="#highlights">Highlights</a>
          <a href="#closer-look">Closer look</a>
          <a href="#local">Local</a>
        </div>
      </footer>

      {commandOpen && (
        <div className="modal-backdrop" role="presentation" onClick={() => setCommandOpen(false)}>
          <div
            className="command-modal"
            role="dialog"
            aria-modal="true"
            aria-label="Command palette"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="command-input modal-input">
              <Search size={17} />
              <span>Search commands and projects</span>
              <button type="button" aria-label="Close command palette" onClick={() => setCommandOpen(false)}>
                <X size={16} />
              </button>
            </div>
            <div className="modal-project">
              <FolderOpen size={16} />
              <span>Current project · crusted-spinning-lynx-agent</span>
            </div>
            <div className="command-list modal-list">
              {commandRows.map(([label, meta, Icon]) => (
                <button key={label} type="button" onClick={() => setCommandOpen(false)}>
                  <Icon size={16} />
                  <span>{label}</span>
                  <em>{meta}</em>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {toast && (
        <div className="toast" role="status">
          <Check size={16} />
          <span>{toast}</span>
        </div>
      )}
    </main>
  );
}

export default App;
