// Frozen Components catalog: shared primitives, area styles and real-screen previews.
// Demonstrations are preview-only; local interactions never mutate engine data.
import * as React from "react";
import { Popover } from "@base-ui/react/popover";
import { Collapsible } from "@base-ui/react/collapsible";
import { Tabs } from "@base-ui/react/tabs";
import { Dialog } from "@base-ui/react/dialog";
import { AlertDialog } from "@base-ui/react/alert-dialog";
import { PreviewCard } from "@base-ui/react/preview-card";
import { Checkbox } from "@base-ui/react/checkbox";
import { Radio } from "@base-ui/react/radio";
import { RadioGroup } from "@base-ui/react/radio-group";
import { Slider } from "@base-ui/react/slider";
import { Meter } from "@base-ui/react/meter";
import { Field } from "@base-ui/react/field";
import { NumberField } from "@base-ui/react/number-field";
import { Icon, Gel, IconBtn, Row, Section, Segmented, ProgressCard, ModeSwitcher, Tip, Switch, Pop, MItem, MSep, useToast, type Mode } from "../../kit/ui";
import { useNav } from "../../shell/nav";
import { Composer } from "../../components/composer";
import { useT, useI18n } from "../../i18n";
import { isPreview, useFixtures } from "../../preview";
import { Mascot, DEFAULT_MASCOT, STATES, COLORS, SHAPE_LIST, type MascotConfig, type State } from "../../mascot/Mascot";
import { SYMBOLS } from "../bots/bot";
import { ACCESSORIES, SLOT_LABEL } from "../../mascot/parts";
import { SCREENS } from "../../registry";
import { FAMILIES, MOTIONS, SECTIONS, type FamilyDef } from "./components-data";
import systemSvg from "../../icons/svg/system.svg?raw";
import { Hairline } from "../bots/hairline";
import "./components.css";

const NB = "\u202f";
const goSection = (id: string) => {
  const heading = document.getElementById("cmp-h-" + id);
  heading?.focus({ preventScroll: true });
  document.getElementById("cmp-" + id)?.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
};

function useCopy() {
  const t = useT();
  const toast = useToast();
  return async (text: string) => {
    try { await navigator.clipboard.writeText(text); toast.add({ title: t("components.copy.001"), data: { icon: "copy" } }); }
    catch { toast.add({ title: t("components.copy.002"), description: t("components.copy.003"), data: { icon: "alert-triangle" } }); }
  };
}

function Block({ title, note, wide, col, children }: { title: string; note?: string; wide?: boolean; col?: boolean; children: React.ReactNode }) {
  const id = React.useId();
  return (
    <section className={"cblock cmp-block" + (wide ? " cmp-wide" : "")} aria-labelledby={id}>
      <div className="cblock-head"><h3 id={id}>{title}</h3>{note && <span className="mono">{note}</span>}</div>
      <div className={"cblock-body" + (col ? " cmp-col" : "")}>{children}</div>
    </section>
  );
}

// Keep thumbnails in sync with the current theme.
const getTheme = () => (document.documentElement.dataset.theme === "light" ? "light" : "dark");
const subTheme = (f: () => void) => { const o = new MutationObserver(f); o.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] }); return () => o.disconnect(); };
const useTheme = () => React.useSyncExternalStore(subTheme, getTheme, () => "dark");

const LiveNear = React.createContext<readonly string[]>([]);
function LiveViewport({ children }: { children: React.ReactNode }) {
  const ref = React.useRef<HTMLDivElement>(null);
  const [near, setNear] = React.useState<string[]>([]);
  React.useEffect(() => {
    const root = ref.current!;
    // ponytail: fixed catalog; reobserve nodes if families become dynamic.
    const nodes = [...root.querySelectorAll(".cmp-live-wrap")];
    const update = () => {
      const r = root.getBoundingClientRect(), center = (r.top + r.bottom) / 2;
      const next = nodes.map((node) => ({ node, box: node.getBoundingClientRect() }))
        .filter(({ box }) => box.bottom >= r.top - 120 && box.top <= r.bottom + 120)
        .sort((a, b) => Math.abs((a.box.top + a.box.bottom) / 2 - center) - Math.abs((b.box.top + b.box.bottom) / 2 - center))
        .slice(0, 3).map(({ node }) => node.id);
      setNear((prev) => prev.length === next.length && prev.every((node, i) => node === next[i]) ? prev : next);
    };
    const observer = new IntersectionObserver(update, { root, rootMargin: "120px 0px" });
    nodes.forEach((node) => observer.observe(node));
    root.addEventListener("scroll", update, { passive: true });
    return () => { observer.disconnect(); root.removeEventListener("scroll", update); };
  }, []);
  return <LiveNear.Provider value={near}><div className="page cmp" ref={ref}>{children}</div></LiveNear.Provider>;
}

// Thumbnails are inert. Full-screen links retain keyboard interaction.
// Unmounting distant previews also releases their timers and animations.
function Live({ id, v, crop = [390, 82, 996, 760], target, height = 260, label }: { id: string; v?: string; crop?: [number, number, number, number]; target?: string; height?: number; label: string }) {
  const t = useT();
  const theme = useTheme();
  const { locale } = useI18n();
  const liveId = React.useId();
  const ref = React.useRef<HTMLDivElement>(null);
  const near = React.useContext(LiveNear).includes(liveId);
  const pending = React.useRef<MutationObserver | null>(null);
  const [width, setWidth] = React.useState(900);
  const [frame, setFrame] = React.useState(crop);
  const [x, y, w, h] = frame, s = Math.min(height / h, width / w, 1);
  React.useEffect(() => {
    const node = ref.current!;
    const resize = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    resize.observe(node);
    return () => resize.disconnect();
  }, []);
  const route = screenFor(id)?.id ?? id;
  const variant = variantFor(id, v);
  const params = { theme, shot: "", ...(variant ? { v: variant } : {}) };
  const src = `#/${route}?${new URLSearchParams(params)}`;
  const { go } = useNav();
  React.useLayoutEffect(() => () => pending.current?.disconnect(), [src, near]);
  return (
    <div className="cmp-live-wrap" id={liveId} ref={ref}>
      <div className="cmp-live" style={{ height, maxWidth: target ? undefined : Math.round(w * s) }}>
        {near ? <div className="cmp-live-crop" style={{ width: w * s, height: h * s }}><iframe key={src + locale} title={label} src={location.pathname + src} width={1440} height={900} tabIndex={-1} inert aria-hidden onLoad={(e) => {
          const iframe = e.currentTarget, doc = iframe.contentDocument;
          if (!doc) return;
          pending.current?.disconnect();
          // PreviewGate loads fixtures asynchronously, after the document load event.
          const ready = () => {
            if (!doc.querySelector(".content")?.childElementCount) return;
            pending.current?.disconnect();
            void doc.fonts.ready.then(() => requestAnimationFrame(() => requestAnimationFrame(() => {
              if (!iframe.isConnected) return;
              const el = target && (doc.querySelector(target) ?? doc.querySelector(".content"));
              if (el) {
                const scroll = el.closest(".page, .thread, .medias-stage");
                if (scroll && scroll !== el) scroll.scrollTop += el.getBoundingClientRect().top - scroll.getBoundingClientRect().top - 16;
                const r = el.getBoundingClientRect(), left = Math.max(0, r.left - 8), top = Math.max(74, r.top - 8);
                setFrame([left, top, Math.max(1, Math.min(1400, r.right + 8) - left), Math.max(1, Math.min(864, r.bottom + 8) - top)]);
                iframe.dataset.target = doc.querySelector(target!) ? target! : ".content";
              } else setFrame(crop);
              iframe.dataset.loaded = "";
            })));
          };
          pending.current = new MutationObserver(ready);
          pending.current.observe(doc, { childList: true, subtree: true });
          ready();
        }} style={{ transform: `translate(${-x * s}px, ${-y * s}px) scale(${s})` }} /></div> : <span className="cmp-cap">{t("components.copy.004")}</span>}
      </div>
      <a className="cmp-source" href={src} onClick={(e) => { e.preventDefault(); go(route, params); }}>{t("components.preview.open", { name: label })}<Icon name="link" /></a>
    </div>
  );
}

const Sec = ({ id, title, children }: { id: string; title: string; children: React.ReactNode }) => (
  <section id={"cmp-" + id} className="cmp-sec" aria-labelledby={"cmp-h-" + id}><h2 id={"cmp-h-" + id} tabIndex={-1}>{title}</h2><div className="cgrid">{children}{FAMILIES.filter((f) => f.section === id).map((f) => <Family key={f.id} family={f} />)}</div></section>
);

function Family({ family: f }: { family: FamilyDef }) {
  const t = useT();
  const [v, setV] = React.useState(() => variantFor(f.route, f.v));
  const screen = screenFor(f.route);
  const variants = screen?.variants ?? [];
  const input = React.useId();
  return <div id={"cmp-family-" + f.id} className="cmp-family cmp-wide" data-family={f.id}>
    <Block title={t(f.title)} note={t("components.asset", { name: f.source })} col>
      <p className="cmp-usage">{t(f.usage)}</p>
      <label className="cmp-variant" htmlFor={input}>{t("components.copy.006")}<select className="input" id={input} value={v} onChange={(e) => setV(e.target.value)}>{variants.map(([id, name]) => <option key={id} value={id}>{t(name)}</option>)}</select></label>
      <Live id={f.route} v={v} target={f.targets?.[variants.find(([id]) => id === v)?.[2] ?? v] ?? f.target} height={460} label={t(f.title)} />
      <span className="cmp-cap">{t("components.family.preview", { route: screen?.id ?? f.route, variant: v })}</span>
      {f.note && <span className="cmp-cap">{t(f.note)}</span>}
    </Block>
  </div>;
}

function FamilyIndex() {
  const t = useT();
  const [q, setQ] = React.useState("");
  const needle = q.trim().toLocaleLowerCase(document.documentElement.lang);
  const found = FAMILIES.filter((f) => [t(f.title), t(f.usage), f.route, f.source].join(" ").toLocaleLowerCase(document.documentElement.lang).includes(needle));
  return <details className="cmp-index">
    <summary>{t("components.familyIndex.title", { count: FAMILIES.length })}</summary>
    <label className="field">{t("components.copy.013")}<input className="input" type="search" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t("components.copy.014")} /></label>
    <nav aria-label={t("components.copy.015")}>{found.map((f) => <button key={f.id} className="cmp-index-link" onClick={() => {
      const el = document.getElementById("cmp-family-" + f.id)!;
      const h = el.querySelector("h3")!; h.tabIndex = -1; h.focus({ preventScroll: true });
      el.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
    }}><span>{t(f.title)}</span><span className="mono">{t("components.family.route", { route: f.route, variant: variantFor(f.route, f.v) })}</span></button>)}</nav>
    <span className="cmp-cap" role="status">{t("components.familyIndex.count", { count: found.length })}</span>
  </details>;
}

function Catalog() {
  const t = useT();
  return (<>
    <div className="content-top"><h1 className="title cmp-title">{t("components.title")}</h1><div className="spacer" /><span className="mono cmp-meta">{t("components.fonts")}</span></div>
    <LiveViewport>
      <p className="cmp-demo-note">{t("components.demoOnly")}</p>
      <nav className="cmp-toc" aria-label={t("components.copy.018")}>{SECTIONS.map(([id, l]) => <button key={id} className="chip" onClick={() => goSection(id)}>{t(l)}</button>)}</nav>
      <FamilyIndex />
      <Sec id="shell" title={t("components.section.shell")}><ShellBlocks /></Sec>
      <Sec id="navigation" title={t("components.section.navigation")}><Navigation /></Sec>
      <Sec id="chat" title={t("components.section.chat")}><ChatBlocks /></Sec>
      <Sec id="bots" title={t("components.section.bots")}><Bots /></Sec>
      <Sec id="work" title={t("components.section.work")}><WorkBlocks /></Sec>
      <Sec id="files" title={t("components.section.files")}><FileBlocks /></Sec>
      <Sec id="code" title={t("components.section.code")}><CodeBlocks /></Sec>
      <Sec id="system" title={t("components.section.system")}><SystemBlocks /></Sec>
      <Sec id="forms" title={t("components.section.forms")}><FormBlocks /></Sec>
      <Sec id="overlays" title={t("components.section.overlays")}><OverlayBlocks /></Sec>
      <Sec id="status" title={t("components.section.status")}><StatusBlocks /></Sec>
      <Sec id="motion" title={t("components.section.motion")}><MotionBlocks /></Sec>
    </LiveViewport>
  </>);
}

function ShellBlocks() {
  const t = useT();
  const { go } = useCatalogNav();
  const [rail, setRail] = React.useState("home");
  const key = (k: string) => dispatchEvent(new KeyboardEvent("keydown", { key: k, ctrlKey: true }));
  return (<>
    <Block title={t("components.copy.019")} note={t("components.copy.020")}>
      <div className="lights"><i /><i /><i /></div>
      <div className="nav-btns"><IconBtn icon="sidebar-left" label={t("components.copy.021")} kbd="⌘B" onClick={() => key("b")} /><IconBtn icon="arrow-left" label={t("components.copy.022")} kbd="⌘[" disabled /><IconBtn icon="arrow-right" label={t("components.copy.023")} kbd="⌘]" disabled /></div>
      <span style={{ flex: 1 }} /><IconBtn icon="share" label={t("components.copy.024")} onClick={() => go("share")} /><IconBtn icon="focus" label={t("components.copy.025")} kbd="⌘\" onClick={() => key("\\")} />
    </Block>
    <Block title={t("components.copy.026")} note={t("components.copy.027")}>
      <div className="cmp-rail">
        <Tip label={t("components.copy.028")} side="right"><button className="rail-btn" aria-label={t("components.copy.028")} aria-pressed={rail === "home"} data-active={rail === "home" || undefined} onClick={() => setRail("home")}><Icon name="home" size={18} /></button></Tip>
        <div className="rail-sep" />
        {([["projects", t("components.copy.030")], ["history", t("components.copy.031")], ["agent", t("components.copy.032")], ["mentions", t("components.copy.033")]] as const).map(([i, l]) => <Tip key={i} label={l} side="right"><button className="rail-btn" aria-label={l} aria-pressed={rail === i} data-active={rail === i || undefined} onClick={() => setRail(i)}><Icon name={i} size={18} /></button></Tip>)}
        <div className="avatar-dot">{t("components.copy.034")}</div>
      </div>
    </Block>
    <Block title={t("components.copy.035")} note={t("components.copy.036")}>
      <Live id="home" crop={[38, 638, 58, 144]} height={144} label={t("components.copy.028")} />
      <button className="btn secondary" onClick={() => document.querySelector<HTMLButtonElement>('.rail .theme [aria-checked="true"]')?.focus()}>{t("components.copy.037")}</button>
      <span className="cmp-cap">{t("components.theme.caption")}</span>
    </Block>
    <Block title={t("components.copy.042")} note={t("components.copy.043")}>
      <button className="btn secondary cmp-focus" onClick={(e) => e.currentTarget.focus()}>{t("components.copy.044")}</button>
      <span className="cmp-focus-ring"><IconBtn icon="search" label={t("components.copy.045")} onClick={() => goSection("system")} /></span>
      <span className="cmp-cap">{t("components.copy.046")}</span>
    </Block>
  </>);
}

const ICON_NAMES = [...new Set([...Object.keys(import.meta.glob("../../icons/svg/*.svg")).map((p) => p.split("/").pop()!.slice(0, -4)), "sidebar-left", "sidebar-right", "more-dots", "system"])].sort();
const GEL_NAMES = Object.keys(import.meta.glob("/public/gel/*.png")).map((p) => p.split("/").pop()!.slice(0, -4)).sort();
function Navigation() {
  const t = useT();
  const { go } = useCatalogNav();
  const [mode, setMode] = React.useState<Mode>("Cortex");
  const [seg, setSeg] = React.useState(t("components.section.chat"));
  const [view, setView] = React.useState(t("components.copy.047"));
  const [row, setRow] = React.useState("bot");
  const [done, setDone] = React.useState(2);
  const [sent, setSent] = React.useState("");
  const bot = useDemoBot();
  return (<>
    <Block title={t("components.copy.048")} note={t("components.copy.049")}>
      <ModeSwitcher mode={mode} onMode={setMode} />
    </Block>
    <Block title={t("components.motion.2.name")} note={t("components.copy.050")}>
      <Segmented items={[t("components.section.chat"), t("components.section.work")]} value={seg} onChange={setSeg} />
      <Segmented items={[t("components.copy.047"), t("components.copy.051")]} value={view} onChange={setView} />
    </Block>
    <Block title={t("components.copy.052")} note={t("components.copy.053")} col>
      <div className="cmp-sb">
        <Section title={t("components.copy.054")} action={<IconBtn icon="plus" label={t("components.copy.055")} onClick={() => go("projects")} />} />
        <Row label={t("components.copy.056")} icon="compose" active={row === "chat"} onClick={() => setRow("chat")} />
        <Row label={bot.name} lead={<Mascot cfg={bot} state="working" size={16} />} meta={t("components.copy.057")} active={row === "bot"} onClick={() => setRow("bot")} />
        <Row label={t("components.copy.058")} gel="recherche-web" active={row === "web"} onClick={() => setRow("web")} />
        <Row label={t("components.copy.059")} icon="folder-open" strong active={row === "project"} onClick={() => setRow("project")} />
        <Row label={t("components.copy.060")} child active={row === "plan"} dim={row !== "plan"} onClick={() => setRow("plan")} actions={<Pop trigger={<IconBtn icon="more-dots" label={t("components.copy.061")} />}><MItem icon="folder" onClick={() => go("project")}>{t("components.copy.062")}</MItem></Pop>} />
        <Row label={t("components.copy.063")} child dim onClick={() => go("project")} />
        <Row label={t("components.copy.064")} icon="folder" strong meta={t("components.copy.065")} status="yellow" active={row === "studio"} onClick={() => setRow("studio")} />
      </div>
    </Block>
    <Block title={t("components.copy.066")} note={t("components.copy.067")} col>
      <div style={{ width: 260, marginLeft: -12 }}><ProgressCard label={t("components.copy.068")} done={done} total={5} onClick={() => setDone((n) => n === 5 ? 0 : n + 1)} /></div>
    </Block>
    <Block title={t("components.copy.069")} note={t("components.copy.070")}>
      <div className="cmp-assets">{GEL_NAMES.map((g) => <figure key={g} className="cmp-fig" data-gel={g}><Gel name={g} size={16} /><figcaption>{t("components.asset", { name: g })}</figcaption></figure>)}</div>
    </Block>
    <Block title={t("components.copy.071")} note={t("components.copy.072", { value1: ICON_NAMES.length + 1 })} wide>
      <div className="cmp-assets cmp-icons">{ICON_NAMES.map((i) => <figure key={i} className="cmp-fig" data-icon={i}><Icon name={i} /><figcaption>{t("components.asset", { name: i })}</figcaption></figure>)}<figure className="cmp-fig" data-icon-source="system"><span className="cmp-icon-svg" aria-hidden dangerouslySetInnerHTML={{ __html: systemSvg }} /><figcaption>{t("components.copy.073")}</figcaption></figure></div>
      <span className="cmp-cap">{t("components.copy.074")}</span>
    </Block>
    <Block title={t("components.copy.075")} note={t("components.copy.076")} wide>
      <div className="cmp-stack" style={{ width: "100%" }}><Composer onSend={setSent} /><span className="cmp-cap" role="status">{sent ? t("components.copy.077", { sent: sent }) : t("components.copy.078")}</span></div>
    </Block>
  </>);
}

function Cite({ n, site, date, t: title, d, c }: { n: number; site: string; date: string; t: string; d: string; c: string }) {
  const t = useT();
  return (<span className="chat-cw">{"\u00a0"}
    <Popover.Root>
      <Popover.Trigger openOnHover delay={150} className="chat-cite" aria-label={t("components.copy.079", { n: n, site: site })}>{n}</Popover.Trigger>
      <Popover.Portal><Popover.Positioner side="top" sideOffset={6}><Popover.Popup className="popup chat-cite-pop">
        <div className="chat-cite-site"><span className="chat-fav" data-c={c} style={{ width: 16, height: 16 }} aria-hidden>{site[0].toUpperCase()}</span>{site}<span>· {date}</span></div>
        <div className="chat-cite-t">{title}</div><div className="chat-cite-d">{d}</div>
      </Popover.Popup></Popover.Positioner></Popover.Portal>
    </Popover.Root></span>);
}

function Att({ name, meta, pct, err, img }: { name: string; meta: string; pct?: number; err?: boolean; img?: string }) {
  const t = useT();
  const L = 2 * Math.PI * 9;
  const [removed, setRemoved] = React.useState(false);
  if (removed) return <button className="btn secondary" onClick={() => setRemoved(false)}>{t("components.copy.080")}{name}</button>;
  return (
    <div className="chat-att" data-err={err || undefined}>
      {img ? <span className="chat-att-img" style={{ backgroundImage: `url(${img})` }} /> : <span className="chat-att-ic">{pct !== undefined
        ? <svg width="22" height="22" viewBox="0 0 22 22" role="progressbar" aria-valuenow={pct} aria-label={t("components.copy.081")}><circle cx="11" cy="11" r="9" fill="none" stroke="var(--subtle)" strokeWidth="2" /><circle cx="11" cy="11" r="9" fill="none" stroke="var(--blue)" strokeWidth="2" strokeLinecap="round" strokeDasharray={`${(L * pct) / 100} ${L}`} transform="rotate(-90 11 11)" className="chat-ring" /></svg>
        : <Icon name={err ? "alert-triangle" : "file"} />}</span>}
      <span className="chat-att-txt"><span className="ttl">{name}</span><span className="sub">{meta}</span></span>
      <IconBtn icon="close" label={t("components.copy.082", { name: name })} className="chat-att-x" onClick={() => setRemoved(true)} />
    </div>
  );
}

function Streaming() {
  const t = useT();
  const STREAM = t("components.streaming.text");
  const { go } = useCatalogNav();
  const bot = useDemoBot();
  const [n, setN] = React.useState(STREAM.length);
  const [on, setOn] = React.useState(false);
  const replay = () => { const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches; setN(reduced ? STREAM.length : 0); setOn(!reduced); };
  React.useEffect(() => {
    if (!on) return;
    const id = setInterval(() => setN((x) => Math.min(STREAM.length, x + 3)), 30);
    return () => clearInterval(id);
  }, [on, STREAM.length]);
  React.useEffect(() => { if (n >= STREAM.length) setOn(false); }, [n, STREAM.length]);
  return (<>
    <div className="msg-bot-row" style={{ width: "100%" }}><Mascot cfg={bot} state={on ? "talking" : "idle"} size={22} /><div className="msg-bot chat-grow"><p style={{ margin: 0, minHeight: 42 }}>{STREAM.slice(0, n)}{on && <span className="chat-caret" />}</p></div></div>
    <div className="chat-box"><form className="composer" onSubmit={(e) => { e.preventDefault(); replay(); }} onKeyDown={(e) => { if (e.key === "Escape") setOn(false); }}>
      <IconBtn type="button" icon="paperclip" label={t("components.copy.083")} className="round" onClick={() => go("upload")} />
      <input placeholder={t("components.copy.084")} aria-label={t("components.copy.084")} />
      <span className="chat-model">{t("components.copy.085")}</span>
      {on ? <Tip label={t("components.copy.086")} kbd={t("components.copy.087")}><button type="button" className="send chat-stop" aria-label={t("components.copy.086")} onClick={() => setOn(false)}><Icon name="stop" size={16} /></button></Tip>
        : <Tip label={t("components.copy.089")}><button type="button" className="send" data-testid="catalog-replay-stream" aria-label={t("components.copy.089")} onClick={() => { replay(); }}><Icon name="refresh" size={16} /></button></Tip>}
    </form></div>
  </>);
}

function ChatBlocks() {
  const t = useT();
  const { go } = useCatalogNav();
  const copy = useCopy();
  const bot = useDemoBot();
  const [ver, setVer] = React.useState(1);
  const [up, setUp] = React.useState("");
  const [vers, setVers] = React.useState([t("components.copy.091"), t("components.copy.092")]);
  const [edit, setEdit] = React.useState(false);
  return (<>
    <Block title={t("components.copy.093")} note={t("components.copy.094")} col>
      <div className="chat-ucol" style={{ maxWidth: "100%" }}>
        {edit ? <form className="cmp-stack" onSubmit={(e) => { e.preventDefault(); setEdit(false); }}><label className="field">{t("components.copy.095")}<textarea className="input" value={vers[ver]} onChange={(e) => setVers(vers.map((text, i) => i === ver ? e.target.value : text))} /></label><button className="btn primary" disabled={!vers[ver].trim()}>{t("components.copy.096")}</button></form> : <div className="msg-user" key={ver}>{vers[ver]}</div>}
        <div className="chat-uact">
          <span className="chat-vers" role="group" aria-label={t("components.copy.097")}>
            <IconBtn icon="chevron-right" label={t("components.copy.098")} className="chat-flip" disabled={ver === 0} onClick={() => setVer(ver - 1)} />
            <span className="chat-meta" aria-live="polite">{ver + 1}/2</span>
            <IconBtn icon="chevron-right" label={t("components.copy.099")} disabled={ver === 1} onClick={() => setVer(ver + 1)} />
          </span>
          <IconBtn icon="copy" label={t("components.copy.100")} onClick={() => copy(vers[ver])} /><IconBtn icon="edit" label={t("components.copy.101")} onClick={() => setEdit(true)} />
        </div>
      </div>
    </Block>
    <Block title={t("components.copy.102")} note={t("components.copy.103")} col>
      <div className="msg-bot-row"><Mascot cfg={bot} state="idle" size={22} /><div className="msg-bot chat-grow">
        <p>{t("components.copy.104")}{NB}%<Cite n={1} c="blue" site={t("components.copy.105")} date={t("components.copy.106")} t={t("components.copy.107")} d={t("components.copy.108")} />{NB}{t("components.copy.109")}<Cite n={2} c="green" site={t("components.copy.110")} date={t("components.copy.111")} t={t("components.copy.112")} d={t("components.copy.113")} />.</p>
        <div className="msg-actions">
          <IconBtn icon="copy" label={t("components.copy.100")} onClick={() => copy(t("components.copy.114"))} />
          <IconBtn icon="thumb-up" label={t("components.copy.115")} aria-pressed={up === "up"} data-on={up === "up" || undefined} onClick={() => setUp(up === "up" ? "" : "up")} />
          <IconBtn icon="thumb-down" label={t("components.copy.116")} aria-pressed={up === "down"} data-on={up === "down" || undefined} onClick={() => setUp(up === "down" ? "" : "down")} />
          <IconBtn icon="refresh" label={t("components.copy.117")} onClick={() => { setUp(""); document.querySelector<HTMLButtonElement>('[data-testid="catalog-replay-stream"]')?.click(); }} /><IconBtn icon="share" label={t("components.copy.024")} onClick={() => go("share")} />
        </div>
      </div></div>
    </Block>
    <Block title={t("components.copy.118")} note={t("components.copy.119")}>
      <Att name={t("components.copy.120")} meta={t("components.copy.121")} />
      <Att name={t("components.copy.122")} meta={t("components.copy.123")} img="/img/ceramique.png" />
      <Att name={t("components.copy.124")} meta={t("components.copy.125")} pct={62} />
      <Att name={t("components.copy.126")} meta={t("components.copy.127")} err />
    </Block>
    <Block title={t("components.copy.128")} note={t("components.copy.129")} col>
      <Collapsible.Root className="chat-reason" defaultOpen>
        <Collapsible.Trigger className="chat-reason-t"><span>{t("components.reasoning.elapsed")}</span><Icon name="chevron-right" size={12} className="chat-chev" /></Collapsible.Trigger>
        <Collapsible.Panel className="chat-reason-p"><ol>{[t("components.copy.133"), t("components.copy.134"), t("components.copy.135")].map((s) => <li key={s}>{s}</li>)}</ol></Collapsible.Panel>
      </Collapsible.Root>
      <span className="chat-reason-t" style={{ alignSelf: "flex-start" }}><span className="thinking">{bot.name}{t("components.copy.136")}</span><span className="chat-meta">6{NB}{t("components.copy.131")}</span></span>
    </Block>
    <Block title={t("components.copy.137")} note={t("components.copy.138")} wide col>
      <Streaming />
    </Block>
  </>);
}

const EYES = ["commas", "dots", "ovals", "pixels"] as const;
const SHAPES_DEMO: MascotConfig[] = SHAPE_LIST.map((shape, i) => ({ name: "", shape, color: COLORS[i][1], eyes: EYES[i % 4], mouth: "none" }));
const ASSET_BOT = SHAPES_DEMO[0];

// Forward keyboard activation to the shared mascot pointer animation.
function MascotPress(props: React.ComponentProps<typeof Mascot>) {
  const t = useT();
  return <button className="cmp-mascot-press" aria-label={t("components.copy.139", { value1: props.cfg.name })} onClick={(e) => { if (e.detail === 0) e.currentTarget.querySelector("svg")?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })); }}><Mascot {...props} interactive /></button>;
}

function Bots() {
  const t = useT();
  const { go } = useCatalogNav();
  const bot = useDemoBot();
  const [st, setSt] = React.useState<State>("idle");
  const [active, setActive] = React.useState(true);
  return (<>
    <Block title={t("components.copy.140")} note={t("components.copy.141")} wide>
      {SHAPES_DEMO.map((c) => <figure key={c.shape} className="cmp-fig"><MascotPress cfg={{ ...c, name: t(`bots.shape.${c.shape}`) }} state="idle" size={56} /><figcaption>{t(`bots.shape.${c.shape}`)}</figcaption></figure>)}
    </Block>
    <Block title={t("components.copy.142")} note={t("components.copy.143")} wide>
      {STATES.map((s) => <figure key={s.id} className="cmp-fig"><span className="cmp-pair"><Mascot cfg={bot} state={s.id} size={44} /><Mascot cfg={bot} state={s.id} size={18} /></span><figcaption>{t(s.label)}</figcaption></figure>)}
    </Block>
    <Block title={t("components.copy.144")} note={t("components.copy.145")} col>
      <div className="cmp-row"><MascotPress cfg={bot} state={st} size={72} track /><span className="cmp-cap" style={{ flexBasis: "auto", flex: 1 }}>{t(STATES.find((s) => s.id === st)!.hint)}</span></div>
      <div className="states" role="group" aria-label={t("components.copy.146")}>{STATES.map((s) => <button key={s.id} aria-label={t(s.label)} aria-pressed={st === s.id} className="state-chip" data-on={st === s.id || undefined} onClick={() => setSt(s.id)}><Mascot cfg={bot} state={s.id} size={18} />{t(s.label)}</button>)}</div>
    </Block>
    {(["glasses", "hat", "extra"] as const).map((slot) => <Block key={slot} title={t("components.copy.147", { value1: t(SLOT_LABEL[slot]) })} note={t("components.copy.148")} wide>
      {ACCESSORIES.filter((a) => a.slot === slot).map((a) => <figure key={a.id} className="cmp-fig" data-accessory={a.id}><Mascot cfg={{ ...ASSET_BOT, [slot]: a.id }} state="idle" size={56} /><figcaption>{t(a.label)}</figcaption></figure>)}
    </Block>)}
    <Block title={t("components.copy.149")} note={t("components.copy.150")} wide>
      {EYES.map((eyes) => <figure key={eyes} className="cmp-fig" data-eyes={eyes}><Mascot cfg={{ ...ASSET_BOT, eyes }} state="idle" size={48} /><figcaption>{t(`bots.eyes.${eyes}`)}</figcaption></figure>)}
      {([['none', 'bots.mouth.none'], ['smile', 'bots.mouth.smile'], ['o', 'bots.mouth.o']] as const).map(([mouth, label]) => <figure key={mouth} className="cmp-fig" data-mouth={mouth}><Mascot cfg={{ ...ASSET_BOT, mouth }} state="idle" size={48} /><figcaption>{t(label)}</figcaption></figure>)}
    </Block>
    <Block title={t("components.copy.151")} note={t("components.copy.152")} wide>
      {COLORS.map(([label, color]) => <figure key={color} className="cmp-fig" data-color={color}><Mascot cfg={{ ...ASSET_BOT, color }} state="idle" size={48} /><figcaption>{t(`bots.color.${label}`)}</figcaption></figure>)}
    </Block>
    <Block title={t("components.copy.153")} note={t("components.copy.154")} wide>
      {SYMBOLS.map((symbol) => <figure key={symbol} className="cmp-fig" data-symbol={symbol}><Mascot cfg={{ ...ASSET_BOT, symbol }} state="idle" size={56} /><figcaption>{t(`bots.symbol.${symbol}`)}</figcaption></figure>)}
    </Block>
    <Block title={t("components.copy.155")} note={t("components.copy.156")}>
      <div className="cmp-sb" style={{ width: 250, padding: 6 }}>
        <Row label={bot.name} lead={<Mascot cfg={bot} state="working" size={16} />} meta={t("components.copy.057")} active={active} onClick={() => setActive(true)} />
        <Row label={bot.name} lead={<Mascot cfg={bot} state="asleep" size={16} />} meta={t("components.copy.157")} active={!active} onClick={() => setActive(false)} />
      </div>
      <button className="roster-item" data-on onClick={() => go("bot")}><Mascot cfg={bot} state="working" size={44} /><span>{bot.name}</span><span className="sub">{t("components.copy.158")}</span></button>
      <button className="roster-item" onClick={() => go("bot-new")}><span className="roster-add"><Icon name="plus" /></span><span>{t("components.copy.160")}</span><span className="sub">{t("components.copy.161")}</span></button>
    </Block>
    <Block title={t("components.copy.162")} note={t("components.copy.163")}>
      <button className="bot-pill" data-on={active || undefined} aria-pressed={active} style={{ marginLeft: 0 }} onClick={() => setActive(!active)}><i />{active ? t("components.copy.164") : t("components.copy.165")}</button>
      <div className="orb" /><Hairline size={96} /><span className="typing" role="img" aria-label={t("components.copy.166")}><i /><i /><i /></span>
    </Block>
  </>);
}

function useDemoTeam(): Record<string, MascotConfig> {
  const t = useT(); return {
  Margaux: { name: t("components.copy.167"), shape: "squircle", color: COLORS[1][1], eyes: "ovals", mouth: "smile", hat: "cap" },
  Max: { name: t("components.copy.168"), shape: "round", color: COLORS[9][1], eyes: "pixels", mouth: "none", glasses: "aviator" },
}; }
function ApprovalCard() {
  const t = useT();
  const team = useDemoTeam();
  const [out, setOut] = React.useState<"" | "ok" | "no">("");
  return (<>
    <div className="travail-appr" data-out={out || undefined} style={{ width: "100%" }}>
      <div inert={!!out}><article className="travail-appr-card" aria-label={t("components.copy.169")}>
        <span className="li-ic"><Icon name="key" size={16} /></span>
        <div className="travail-grow">
          <span className="travail-meta" style={{ display: "flex", alignItems: "center", gap: 6 }}><Mascot cfg={team.Max} state="waiting" size={16} />{t("components.copy.171")}</span>
          <span className="ttl">{t("components.copy.172")}{NB}€</span>
          <span className="sub">{t("components.copy.173")}</span>
          <div className="travail-actions">
            <button className="btn primary" onClick={() => setOut("ok")}><Icon name="check" size={16} />{t("components.copy.175")}</button>
            <button className="btn secondary" onClick={() => setOut("no")}>{t("components.copy.176")}</button>
          </div>
        </div>
        {out && <span className={"badge travail-stamp " + (out === "ok" ? "ok" : "err")}><Icon name={out === "ok" ? "check" : "close"} size={16} />{out === "ok" ? t("components.copy.177") : t("components.copy.178")}</span>}
      </article></div>
    </div>
    {out && <><span role="status" className="cmp-cap">{out === "ok" ? t("components.copy.179") : t("components.copy.180")}</span><button className="btn secondary" onClick={() => setOut("")}><Icon name="refresh" size={16} />{t("components.copy.181")}</button></>}
  </>);
}

function WorkBlocks() {
  const t = useT();
  const team = useDemoTeam();
  const { go } = useCatalogNav();
  const bot = useDemoBot();
  const [mail, setMail] = React.useState<"ready" | "sent" | "cancelled">("ready");
  return (<>
    <Block title={t("components.copy.182")} note={t("components.copy.183")} col>
      <div className="travail-col" style={{ minHeight: 0 }}>
        <div className="travail-col-head"><span className="travail-dot" data-s="run" />{t("components.copy.184")}<span className="travail-meta">2</span></div>
        <div className="travail-kcard">
          <button className="cmp-card-action" aria-label={t("components.copy.185")} onClick={() => go("work-task")} />
          <span className="travail-kcard-t" style={{ paddingRight: 18 }}>{t("components.copy.186")}</span>
          <span className="travail-bar" aria-hidden><i style={{ width: "57%" }} /></span>
          <span className="travail-kcard-f"><Mascot cfg={bot} state="working" size={18} /><span className="travail-grow">{bot.name}{t("components.copy.187")}</span></span>
          <Pop trigger={<button className="ibtn travail-kmenu" aria-label={t("components.copy.188")}><Icon name="more-dots" size={16} /></button>}><MItem icon="arrow-right" onClick={() => go("work-task")}>{t("components.copy.190")}</MItem></Pop>
        </div>
        <div className="travail-kcard" data-drag style={{ translate: "14px 4px" }} aria-hidden>
          <span className="travail-kcard-t">{t("components.copy.191")}</span>
          <span className="travail-kcard-f"><Mascot cfg={team.Margaux} state="working" size={18} /><span className="travail-grow">{t("components.copy.192")}</span></span>
        </div>
      </div>
      <span className="cmp-cap">{t("components.copy.193")}</span>
    </Block>
    <Block title={t("components.copy.194")} note={t("components.copy.195")} col>
      <Live id="work-home" v="tableau" crop={[560, 160, 880, 440]} height={240} label={t("components.copy.196")} />
    </Block>
    <Block title={t("components.copy.197")} note={t("components.copy.198")} wide>
      <div className="travail-widget" data-hl={mail === "ready" || undefined} data-done={mail !== "ready" || undefined} style={{ width: "100%" }}>
        <div className="travail-widget-h"><span className="travail-grow"><Icon name="mail" size={16} />{t("components.copy.200")}</span>
          {mail === "ready" ? <span className="badge wait">{t("components.copy.201")}</span> : <span className="badge ok"><Icon name="check" size={16} />{mail === "sent" ? t("components.copy.202") : t("components.copy.203")}</span>}</div>
        <div className="travail-widget-b">
          <div className="travail-mailmeta"><span>{t("components.copy.204")}</span><b>{t("components.copy.205")}</b><span>{t("components.copy.206")}</span><b>{t("components.copy.207")}</b></div>
          <p>{t("components.copy.208")}{NB}?</p>
        </div>
        <div className="travail-widget-f">
          {mail === "ready" ? <><button className="btn primary" onClick={() => setMail("sent")}><Icon name="arrow-up" size={16} />{t("components.copy.210")}</button><button className="btn secondary" onClick={() => setMail("cancelled")}>{t("components.copy.211")}</button><span className="travail-grow" /><IconBtn icon="edit" label={t("components.copy.212")} onClick={() => go("work-task")} /></>
            : <><span className="travail-grow" role="status">{mail === "sent" ? t("components.copy.213", { value1: bot.name }) : t("components.copy.214")}</span><button className="btn secondary" onClick={() => setMail("ready")}>{t("components.copy.215")}</button></>}
        </div>
      </div>
    </Block>
    <Block title={t("components.copy.216")} note={t("components.copy.217")} col>
      <div className="travail-ev" role="note"><span className="travail-evic"><Icon name="globe" size={16} /></span><span><b>{t("components.copy.219")}</b>{t("components.copy.220")}</span><span className="travail-evline" /><span className="travail-meta">10:38</span></div>
      <div className="travail-ev" role="note"><span className="travail-evic"><Icon name="clock-loop" size={16} /></span><span><b>{t("components.copy.222")}</b>{t("components.copy.223")}</span><span className="travail-evline" /><span className="travail-meta">10:41</span></div>
    </Block>
    <Block title={t("components.copy.224")} note={t("components.copy.225")} col>
      <ApprovalCard />
    </Block>
    <Block title={t("components.motion.3.name")} note={t("components.copy.226")}>
      <Tabs.Root defaultValue="unread" style={{ width: "100%" }}>
        <Tabs.List className="travail-tabs" aria-label={t("components.copy.227")} style={{ marginBottom: 0 }}>
          {([["unread", t("components.copy.228"), 4], ["all", t("components.copy.229"), 0], ["mentions", t("components.copy.230"), 2]] as const).map(([id, l, n]) => <Tabs.Tab key={id} value={id} className="travail-tab">{l}{!!n && <span className="travail-n">{n}</span>}</Tabs.Tab>)}
          <Tabs.Indicator className="travail-tab-ind" />
        </Tabs.List>
        <Tabs.Panel value="unread" className="cmp-cap">{t("components.copy.231")}</Tabs.Panel><Tabs.Panel value="all" className="cmp-cap">{t("components.copy.232")}</Tabs.Panel><Tabs.Panel value="mentions" className="cmp-cap">{t("components.copy.233")}</Tabs.Panel>
      </Tabs.Root>
    </Block>
  </>);
}

function FileBlocks() {
  const t = useT();
  const { go } = useCatalogNav();
  const Z = [0.75, 1, 1.25, 1.5];
  const [z, setZ] = React.useState(1);
  const [pg, setPg] = React.useState(0);
  const i = Z.indexOf(z);
  return (<>
    <Block title={t("components.copy.234")} note={t("components.copy.235")} wide>
      <div className="content-top fichiers-top" style={{ width: "100%", padding: 0, height: 44 }}>
        <span className="fichiers-ftype" data-k="pdf" aria-hidden>{t("components.copy.236")}</span>
        <span className="fichiers-fname fichiers-grow"><span className="ttl fichiers-ell">{t("components.copy.237")}</span><span className="fichiers-meta">{t("components.file.metadata")}</span></span>
        <IconBtn icon="download" label={t("components.copy.240")} onClick={() => go("file-pdf")} /><IconBtn icon="share" label={t("components.copy.024")} onClick={() => go("share")} />
        <button className="btn secondary fichiers-ask" onClick={() => go("file-pdf")}><Icon name="sparkle-free" />{t("components.copy.242")}</button>
        <Pop trigger={<IconBtn icon="more-dots" label={t("components.copy.243")} />}><MItem icon="folder" onClick={() => go("project")}>{t("components.copy.244")}</MItem></Pop>
      </div>
    </Block>
    <Block title={t("components.copy.245")} note={t("components.copy.246")}>
      <div className="fichiers-tools" style={{ border: 0, padding: 0 }}>
        <Tip label={t("components.copy.247")} kbd="⌘−"><button className="ibtn fichiers-glyph" aria-label={t("components.copy.247")} disabled={i === 0} onClick={() => setZ(Z[i - 1])}>−</button></Tip>
        <button className="fichiers-zoom" aria-label={t("components.copy.248")} onClick={() => setZ(1)}>{Math.round(z * 100)}{NB}%</button>
        <Tip label={t("components.copy.249")} kbd="⌘+"><button className="ibtn fichiers-glyph" aria-label={t("components.copy.249")} disabled={i === Z.length - 1} onClick={() => setZ(Z[i + 1])}>+</button></Tip>
        <span className="fichiers-sep" />
        <span className="fichiers-pnum"><IconBtn icon="arrow-left" label={t("components.copy.250")} disabled={pg === 0} onClick={() => setPg(pg - 1)} /><input type="number" min={1} max={12} value={pg + 1} onChange={(e) => { const n = e.target.valueAsNumber; if (Number.isInteger(n) && n >= 1 && n <= 12) setPg(n - 1); }} aria-label={t("components.copy.251")} /><span>{t("components.copy.252")}</span><IconBtn icon="arrow-right" label={t("components.copy.253")} disabled={pg === 11} onClick={() => setPg(pg + 1)} /></span>
      </div>
    </Block>
    <Block title={t("components.copy.254")} note={t("components.copy.255")}>
      <div className="fichiers-root" style={{ flex: "none", flexDirection: "row", gap: 8 }}>
        {[0, 1, 2].map((k) => <button key={k} className="fichiers-thumb" style={{ width: 96 }} data-on={k === pg || undefined} aria-pressed={k === pg} aria-label={t("components.copy.256", { value1: k + 1 })} onClick={() => setPg(k)}>
          <span className="fichiers-mini"><i className="h" /><i style={{ width: "86%" }} /><i style={{ width: "72%" }} />{k === 2 && <i className="b" />}<i style={{ width: "50%" }} /></span>{k + 1}</button>)}
      </div>
    </Block>
    <Block title={t("components.copy.257")} note={t("components.copy.258")} col>
      <Live id="file-image" v="comparaison" crop={[400, 150, 1000, 700]} height={240} label={t("components.copy.259")} />
    </Block>
  </>);
}

function CodeBlocks() {
  const t = useT();
  const copy = useCopy();
  const [applied, setApplied] = React.useState(false);
  const [step, setStep] = React.useState(0);
  const [decision, setDecision] = React.useState("");
  const [ci, setCi] = React.useState("err");
  const before = t("components.code.before");
  const after = t("components.code.after");
  const logs = [["cmd", t("components.copy.260")], ["dim", t("components.copy.261")], ["ok", t("components.copy.262")]];
  return (<>
    <Block title={t("components.copy.263")} note={t("components.copy.264")} wide col>
      <div className="diff code-file">
        <div className="code-head"><Icon name="file-code" /><span>{t("components.copy.266")}</span><span className="code-delta"><span className="code-plus">+1</span><span className="code-minus">−1</span></span><IconBtn icon="copy" label={t("components.copy.267")} onClick={() => copy("./src/billing/invoices.ts")} /></div>
        <div className="code-dv" tabIndex={0} role="region" aria-label={t("components.copy.268")}>
          <div className="code-dl" data-k="hunk"><span className="code-hunk">@@ -41,1 +41,1 @@</span></div>
          {!applied && <div className="code-dl" data-k="del"><span className="code-ln">41</span><span className="code-ln" /><span className="code-sg">−</span><span className="code-src">{before}</span></div>}
          <div className="code-dl" data-k="add"><span className="code-ln" /><span className="code-ln">41</span><span className="code-sg">+</span><span className="code-src">{after}</span></div>
        </div>
      </div>
      <div className="cmp-row"><button className="btn primary" onClick={() => setApplied(!applied)}><Icon name={applied ? "refresh" : "check"} />{applied ? t("components.copy.269") : t("components.copy.270")}</button><span className="cmp-cap" role="status">{applied ? t("components.copy.271") : t("components.copy.272")}</span></div>
    </Block>
    <Block title={t("components.copy.273")} note={t("components.copy.274")} col>
      <div className="term code-term" role="log" aria-label={t("components.copy.275")} aria-live="polite">{logs.slice(0, step + 1).map(([k, text]) => <div key={k} className="code-tl" data-k={k}>{text}</div>)}</div>
      <button className="btn secondary" onClick={() => setStep((n) => (n + 1) % logs.length)}><Icon name={step === 2 ? "refresh" : "play"} />{step === 2 ? t("components.copy.276") : t("components.copy.277")}</button>
    </Block>
    <Block title={t("components.copy.278")} note={t("components.copy.279")} col>
      <div className="code-check" data-s={ci}><span className="code-ci" data-s={ci}>{ci === "run" ? <span className="spin" /> : <Icon name={ci === "ok" ? "check-circle" : "x-circle"} />}</span><span className="code-grow"><span className="ttl">{t("components.copy.280")}</span><span className="sub mono">{t("components.copy.281")}</span></span><span className="code-meta" role="status">{ci === "ok" ? t("components.copy.282") : ci === "run" ? t("components.copy.184") : t("components.copy.283")}</span></div>
      {ci === "err" && <pre className="code-cilog">{t("components.copy.284")}</pre>}
      <button className="btn secondary" onClick={() => setCi(ci === "err" ? "run" : ci === "run" ? "ok" : "err")}>{ci === "err" ? t("components.copy.285") : ci === "run" ? t("components.copy.286") : t("components.copy.287")}</button>
    </Block>
    <Block title={t("components.copy.288")} note={t("components.copy.289")} wide col>
      <div className="code-ask">
        <div className="code-ask-h"><Icon name="shield-check" /><span>{t("components.copy.291")}<code>{t("components.copy.292")}</code></span></div>
        <p>{t("components.copy.293")}<code>{t("components.copy.294")}</code>{t("components.copy.295")}</p>
        {decision ? <div className="code-row-gap"><span role="status">{decision}</span><button className="btn secondary" onClick={() => setDecision("")}>{t("components.copy.296")}</button></div> : <div className="code-row-gap"><button className="btn primary" onClick={() => setDecision(t("components.copy.297"))}>{t("components.copy.298")}</button><button className="btn secondary" onClick={() => setDecision(t("components.copy.299"))}>{t("components.copy.300")}</button></div>}
      </div>
    </Block>
  </>);
}

function SystemBlocks() {
  const t = useT();
  const { go } = useCatalogNav();
  const [otp, setOtp] = React.useState("");
  const [result, setResult] = React.useState("");
  const [q, setQ] = React.useState("");
  const [bill, setBill] = React.useState("annual");
  const hits = [["folder", t("components.copy.059"), t("components.copy.301"), "project"], ["file", "Plan-lancement-printemps.pdf", t("components.copy.302"), "file-pdf"], ["bot", t("components.copy.303"), t("components.copy.304"), "bot"]];
  const found = hits.filter((h) => h[1].toLocaleLowerCase(document.documentElement.lang).includes(q.trim().toLocaleLowerCase(document.documentElement.lang)));
  return (<>
    <Block title={t("components.motion.25.name")} note={t("components.copy.305")} col>
      <form className="cmp-stack" onSubmit={(e) => { e.preventDefault(); setResult(otp === "482915" ? t("components.copy.306") : t("components.copy.307")); }}>
        <div className="systeme-otp" data-err={(result !== "" && otp !== "482915") || undefined}>
          {Array.from({ length: 6 }, (_, i) => <span key={i} className="systeme-cell" aria-hidden data-filled={otp[i] ? "" : undefined} data-cur={i === Math.min(otp.length, 5) || undefined}>{otp[i] ?? ""}</span>)}
          <input value={otp} inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{6}" maxLength={6} required aria-label={t("components.copy.308")} aria-describedby="cmp-otp-help" aria-invalid={(result !== "" && otp !== "482915") || undefined} onChange={(e) => { setOtp(e.target.value.replace(/\D/g, "").slice(0, 6)); setResult(""); }} onPaste={(e) => { e.preventDefault(); setOtp(e.clipboardData.getData("text").replace(/\D/g, "").slice(0, 6)); setResult(""); }} />
        </div>
        <span className="cmp-cap" id="cmp-otp-help">{t("components.otp.help")}</span>
        <button className="btn primary" disabled={otp.length !== 6}>{t("components.copy.311")}</button><span className="cmp-cap" role="status">{result}</span>
      </form>
    </Block>
    <Block title={t("components.copy.312")} note={t("components.copy.313")} col>
      <label className="systeme-search"><Icon name="search" /><input value={q} onChange={(e) => setQ(e.target.value)} placeholder={t("components.copy.315")} aria-label={t("components.copy.315")} />{q && <button className="systeme-clear" aria-label={t("components.copy.316")} onClick={() => setQ("")}><Icon name="close" /></button>}</label>
      <span className="cmp-cap" role="status">{t("components.search.count", { count: found.length })}</span>
      {found.map(([icon, title, sub, route]) => <button key={route} className="systeme-hit" onClick={() => go(route)}><span className="systeme-ic"><Icon name={icon} /></span><span className="systeme-grow"><span className="systeme-t">{title}</span><span className="systeme-s">{sub}</span></span></button>)}
      {!found.length && <button className="btn secondary" onClick={() => setQ("")}>{t("components.copy.318")}</button>}
    </Block>
    <Block title={t("components.copy.319")} note={t("components.copy.320")} col>
      <CatalogPalette />
      <div className="systeme-hint"><span><span className="systeme-keys"><kbd className="systeme-kbd">↑</kbd><kbd className="systeme-kbd">↓</kbd></span>{t("components.copy.321")}</span><span><kbd className="systeme-kbd">↵</kbd>{t("components.copy.322")}</span><span><kbd className="systeme-kbd">{t("components.copy.087")}</kbd>{t("components.copy.323")}</span></div>
      <span className="cmp-cap">{t("components.copy.324")}</span>
    </Block>
    <Block title={t("components.copy.325")} note={t("components.copy.326")} col>
      <Segmented items={[t("components.billing.monthly"), t("components.billing.annual")]} value={t(`components.billing.${bill}`)} onChange={(value) => setBill(value === t("components.billing.annual") ? "annual" : "monthly")} />
      <article className="systeme-plan" data-pop><h3>{t("components.copy.327")}<span className="badge run">{t("components.copy.328")}</span></h3><div className="systeme-desc">{t("components.copy.329")}</div><div className="systeme-price" aria-live="polite"><b key={bill}>{bill === "annual" ? 20 : 24}{NB}€</b><span>{t("components.copy.330")}</span></div><div className="systeme-per">{bill === "annual" ? t("components.copy.331") : t("components.copy.332")}</div><button className="btn primary" onClick={() => go("pricing")}>{t("components.copy.333")}</button><ul className="systeme-feats">{[t("components.copy.334"), t("components.copy.335"), t("components.copy.336"), t("components.copy.337")].map((text) => <li key={text}><Icon name="check" />{text}</li>)}</ul></article>
    </Block>
  </>);
}

function CatalogPalette() {
  const t = useT();
  const [open, setOpen] = React.useState(false);
  const [q, setQ] = React.useState("");
  const [active, setActive] = React.useState(0);
  const list = SECTIONS.filter(([, title]) => t(title).toLocaleLowerCase(document.documentElement.lang).includes(q.toLocaleLowerCase(document.documentElement.lang)));
  const destination = React.useRef<string | null>(null);
  const pick = (index: number) => { if (!list[index]) return; destination.current = list[index][0]; setOpen(false); goSection(list[index][0]); };
  return <Dialog.Root open={open} onOpenChange={setOpen}>
    <Dialog.Trigger className="btn secondary" onClick={() => { destination.current = null; }}><Icon name="command" />{t("components.copy.339")}</Dialog.Trigger>
    <Dialog.Portal><Dialog.Backdrop className="systeme-backdrop" /><Dialog.Popup className="systeme-cmd" finalFocus={() => destination.current ? document.getElementById("cmp-h-" + destination.current) : true}>
      <Dialog.Title className="systeme-sr">{t("components.copy.340")}</Dialog.Title><Dialog.Description className="systeme-sr">{t("components.copy.341")}</Dialog.Description>
      <div className="systeme-cmd-in"><Icon name="search" /><input aria-label={t("components.copy.342")} role="combobox" aria-expanded aria-controls="cmp-command-list" aria-activedescendant={list[active] ? `cmp-command-${list[active][0]}` : undefined} value={q} onChange={(e) => { setQ(e.target.value); setActive(0); }} onKeyDown={(e) => {
        if (!list.length) return;
        if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); const next = (active + (e.key === "ArrowDown" ? 1 : -1) + list.length) % list.length; setActive(next); document.getElementById(`cmp-command-${list[next][0]}`)?.scrollIntoView({ block: "nearest" }); }
        if (e.key === "Enter") { e.preventDefault(); pick(active); }
      }} /><Dialog.Close className="ibtn" aria-label={t("components.copy.343")}><Icon name="close" /></Dialog.Close></div>
      <div className="systeme-cmd-list" id="cmp-command-list" role="listbox" aria-label={t("components.copy.344")}>{list.map(([id, title], i) => <button key={id} id={`cmp-command-${id}`} role="option" tabIndex={-1} aria-selected={active === i} data-active={active === i || undefined} className="systeme-cmd-item" onMouseMove={() => setActive(i)} onClick={() => pick(i)}><Icon name="arrow-right" /><span className="systeme-grow">{t(title)}</span><kbd className="systeme-kbd">↵</kbd></button>)}</div>
      {!list.length && <div className="systeme-cmd-empty" role="status">{t("components.palette.empty")}</div>}
    </Dialog.Popup></Dialog.Portal>
  </Dialog.Root>;
}

function FormBlocks() {
  const t = useT();
  const [checks, setChecks] = React.useState([true, false]);
  const [radio, setRadio] = React.useState("private");
  const [budget, setBudget] = React.useState(45);
  const [email, setEmail] = React.useState("camille@");
  const [invalid, setInvalid] = React.useState(true);
  const [saved, setSaved] = React.useState(false);
  const all = checks.every(Boolean), some = checks.some(Boolean);
  return (<>
    <Block title={t("components.copy.347")} note={t("components.copy.348")} col>
      <label className="cmp-row"><Checkbox.Root className="travail-check" checked={all} indeterminate={some && !all} onCheckedChange={(v) => setChecks([v, v])}><Checkbox.Indicator>{all ? <Icon name="check" /> : <span className="cmp-mixed" />}</Checkbox.Indicator></Checkbox.Root>{t("components.copy.349")}</label>
      {[t("components.copy.350"), t("components.copy.351")].map((text, i) => <label className="cmp-row" key={text}><Checkbox.Root className="travail-check" checked={checks[i]} onCheckedChange={(v) => setChecks(checks.map((old, n) => n === i ? v : old))}><Checkbox.Indicator><Icon name="check" /></Checkbox.Indicator></Checkbox.Root>{text}</label>)}
      <label className="cmp-row"><Checkbox.Root className="travail-check" disabled><Checkbox.Indicator><Icon name="check" /></Checkbox.Indicator></Checkbox.Root>{t("components.copy.352")}</label>
    </Block>
    <Block title={t("components.copy.353")} note={t("components.copy.354")} col>
      <RadioGroup className="code-radios" aria-label={t("components.copy.355")} value={radio} onValueChange={setRadio}>{[["private", t("components.copy.356"), t("components.copy.357")], ["team", t("components.copy.358"), t("components.copy.359")]].map(([id, title, sub]) => <label key={id} className="code-radio-row"><Radio.Root value={id} className="code-radio"><Radio.Indicator className="code-radio-i" /></Radio.Root><span className="code-grow"><span className="ttl">{title}</span><span className="sub">{sub}</span></span></label>)}</RadioGroup>
    </Block>
    <Block title={t("components.copy.360")} note={t("components.copy.361")} col>
      <Slider.Root value={budget} onValueChange={(value) => setBudget(value as number)} min={10} max={100} step={5}>
        <div className="code-meter-l"><span>{t("components.copy.362")}</span><span>{budget}{NB}€</span></div><Slider.Control className="chat-sl-ctl"><Slider.Track className="chat-sl-track"><Slider.Indicator className="chat-sl-ind" /><Slider.Thumb className="chat-sl-thumb" aria-label={t("components.copy.363")} /></Slider.Track></Slider.Control>
      </Slider.Root>
      <Meter.Root className="code-meter" value={budget} min={0} max={100}><div className="code-meter-l"><Meter.Label>{t("components.copy.364")}</Meter.Label><Meter.Value>{() => t("components.copy.365", { budget: budget })}</Meter.Value></div><Meter.Track className="code-meter-t" data-tone={budget >= 80 ? "wait" : "ok"}><Meter.Indicator className="code-meter-i" /></Meter.Track></Meter.Root>
    </Block>
    <Block title={t("components.copy.366")} note={t("components.copy.367")} col>
      <form className="cmp-stack" onSubmit={(e) => { e.preventDefault(); if (!invalid) setSaved(true); }}>
        <Field.Root className="field" invalid={invalid}><Field.Label>{t("components.copy.368")}</Field.Label><Field.Control className="input" type="email" required value={email} onChange={(e) => { setEmail(e.target.value); setInvalid(!e.target.validity.valid); setSaved(false); }} /><Field.Description className="cmp-cap">{t("components.copy.369")}</Field.Description><Field.Error match={invalid} className="systeme-err">{t("components.copy.370")}</Field.Error></Field.Root>
        <button className="btn primary" disabled={invalid}>{t("components.copy.371")}</button><span role="status" className="cmp-cap">{saved ? t("components.copy.372") : ""}</span>
      </form>
    </Block>
    <Block title={t("components.copy.373")} note={t("components.copy.374")} col>
      <NumberField.Root id="cmp-parallel" defaultValue={3} min={1} max={10} step={1} className="field"><label htmlFor="cmp-parallel">{t("components.copy.375")}</label><NumberField.Group className="cmp-row"><NumberField.Decrement className="ibtn" aria-label={t("components.copy.376")}><Icon name="chevron-down" /></NumberField.Decrement><NumberField.Input className="input cmp-number" /><NumberField.Increment className="ibtn" aria-label={t("components.copy.378")}><Icon name="plus" /></NumberField.Increment></NumberField.Group><span className="cmp-cap">{t("components.copy.379")}</span></NumberField.Root>
    </Block>
    <Block title={t("components.copy.380")} note={t("components.copy.381")} col>
      <label className="cmp-row"><Switch defaultChecked aria-label={t("components.copy.382")} />{t("components.copy.382")}</label>
      <div className="cmp-row"><button className="btn primary" onClick={() => goSection("overlays")}>{t("components.copy.383")}</button><button className="btn secondary" onClick={() => goSection("status")}>{t("components.copy.384")}</button><button className="btn secondary" disabled>{t("components.copy.385")}</button></div>
    </Block>
  </>);
}

function OverlayBlocks() {
  const t = useT();
  const toast = useToast();
  const [name, setName] = React.useState(t("components.copy.059"));
  const [draft, setDraft] = React.useState(name);
  const [open, setOpen] = React.useState(false);
  const [pinned, setPinned] = React.useState(false);
  const [preview, setPreview] = React.useState(false);
  const theme = useTheme();
  const { go } = useCatalogNav();
  return (<>
    <Block title={t("components.motion.27.name")} note={t("components.copy.386")} col>
      <Dialog.Root open={open} onOpenChange={(value) => { setOpen(value); if (value) setDraft(name); }}><Dialog.Trigger className="btn secondary">{t("components.copy.387")}</Dialog.Trigger><Dialog.Portal><Dialog.Backdrop className="backdrop" /><Dialog.Popup className="dialog"><Dialog.Title>{t("components.copy.388")}</Dialog.Title><Dialog.Description>{t("components.copy.389")}</Dialog.Description><form onSubmit={(e) => { e.preventDefault(); if (!draft.trim()) return; setName(draft.trim()); setOpen(false); }}><label className="field">{t("components.copy.390")}<input className="input" required maxLength={48} value={draft} onChange={(e) => setDraft(e.target.value)} /></label><div className="systeme-actions"><Dialog.Close type="button" className="btn secondary">{t("components.copy.211")}</Dialog.Close><button className="btn primary" disabled={!draft.trim()}>{t("components.copy.391")}</button></div></form></Dialog.Popup></Dialog.Portal></Dialog.Root>
      <span className="cmp-cap" role="status">{name}</span>
    </Block>
    <Block title={t("components.copy.392")} note={t("components.copy.393")} col>
      <AlertDialog.Root><AlertDialog.Trigger className="btn secondary">{t("components.copy.394")}</AlertDialog.Trigger><AlertDialog.Portal><AlertDialog.Backdrop className="backdrop" /><AlertDialog.Popup className="dialog"><AlertDialog.Title>{t("components.reset.title")}</AlertDialog.Title><AlertDialog.Description>{t("components.reset.description")}</AlertDialog.Description><div className="systeme-actions"><AlertDialog.Close className="btn secondary">{t("components.copy.397")}</AlertDialog.Close><AlertDialog.Close className="btn primary" onClick={() => setName(t("components.copy.059"))}>{t("components.copy.398")}</AlertDialog.Close></div></AlertDialog.Popup></AlertDialog.Portal></AlertDialog.Root>
      <span className="cmp-cap">{t("components.copy.399")}</span>
    </Block>
    <Block title={t("components.copy.400")} note={t("components.copy.401")} col>
      <Pop trigger={<button className="btn secondary">{t("components.copy.402")}<Icon name="chevron-down" size={12} /></button>} width={230}><MItem icon="edit" onClick={() => { setDraft(name); setOpen(true); }}>{t("components.copy.403")}</MItem><MItem icon="pin" onClick={() => setPinned(!pinned)}>{pinned ? t("components.copy.404") : t("components.copy.405")}</MItem><MSep /><MItem icon="folder" onClick={() => goSection("navigation")}>{t("components.copy.406")}</MItem></Pop><span className="cmp-cap" role="status">{pinned ? t("components.copy.407") : t("components.copy.408")}</span>
    </Block>
    <Block title={t("components.copy.409")} note={t("components.copy.410")} col>
      <Popover.Root><Popover.Trigger className="btn secondary">{t("components.copy.411")}</Popover.Trigger><Popover.Portal><Popover.Positioner sideOffset={6}><Popover.Popup className="popup cmp-popup"><Popover.Title className="h3">{t("components.copy.412")}</Popover.Title><Popover.Description className="cmp-cap">{t("components.copy.413")}</Popover.Description><label className="cmp-row"><Switch defaultChecked aria-label={t("components.copy.414")} />{t("components.copy.230")}</label><Popover.Close className="btn secondary">{t("components.copy.415")}</Popover.Close></Popover.Popup></Popover.Positioner></Popover.Portal></Popover.Root>
    </Block>
    <Block title={t("components.copy.416")} note={t("components.copy.417")} col>
      <PreviewCard.Root open={preview} onOpenChange={setPreview}><PreviewCard.Trigger className="cmp-source" href={`#/project?theme=${theme}&shot`} onClick={(e) => { e.preventDefault(); go("project"); }}>{t("components.copy.418")}</PreviewCard.Trigger><PreviewCard.Portal><PreviewCard.Positioner sideOffset={8}><PreviewCard.Popup className="popup cmp-popup"><div className="cmp-row"><Icon name="folder" /><strong>{t("components.copy.059")}</strong></div><p className="cmp-cap">{t("components.copy.420")}</p></PreviewCard.Popup></PreviewCard.Positioner></PreviewCard.Portal></PreviewCard.Root>
      <button className="btn secondary" aria-expanded={preview} onClick={() => setPreview(!preview)}>{t("components.copy.421")}</button>
    </Block>
    <Block title={t("components.copy.422")} note={t("components.copy.423")} col>
      <div className="cmp-row"><button className="btn secondary" onClick={() => toast.add({ title: t("components.copy.424"), description: t("components.copy.425"), data: { icon: "check-circle" } })}>{t("components.copy.426")}</button><button className="btn secondary" onClick={() => toast.add({ title: t("components.copy.427"), description: t("components.copy.428"), data: { icon: "alert-triangle" } })}>{t("components.copy.429")}</button><button className="btn secondary" onClick={() => { const old = pinned; setPinned(true); toast.add({ title: t("components.copy.430"), data: { icon: "pin", undo: true, onUndo: () => setPinned(old) } }); }}>{t("components.copy.431")}</button></div>
    </Block>
  </>);
}

function StatusBlocks() {
  const t = useT();
  const { go } = useCatalogNav();
  const [loading, setLoading] = React.useState(true);
  const [empty, setEmpty] = React.useState(true);
  const [error, setError] = React.useState(true);
  const [offline, setOffline] = React.useState(true);
  const bot = useDemoBot();
  return (<>
    <Block title={t("components.copy.432")} note={t("components.copy.433")} col>
      <div className="cmp-stack" aria-busy={loading} aria-label={t("components.copy.434")}>{loading ? <div className="cmp-row" aria-hidden><span className="skel circle" style={{ width: 32, height: 32 }} /><div className="cmp-stack" style={{ flex: 1 }}><span className="skel title" /><span className="skel line" style={{ width: "80%" }} /><span className="skel line" style={{ width: "60%" }} /></div></div> : <div className="li"><span className="li-ic"><Icon name="file" /></span><span className="grow"><span className="ttl">{t("components.copy.237")}</span><span className="sub">{t("components.copy.436")}</span></span></div>}</div>
      <button className="btn secondary" onClick={() => setLoading(!loading)}>{loading ? t("components.copy.437") : t("components.copy.438")}</button>
    </Block>
    <Block title={t("components.copy.439")} note={t("components.copy.440")}>
      <span className="badge ok"><Icon name="check" />{t("components.copy.415")}</span><span className="badge run"><span className="spin" aria-hidden />{t("components.copy.184")}</span><span className="badge wait">{t("components.copy.441")}</span><span className="badge err"><Icon name="alert-triangle" />{t("components.copy.283")}</span>
      <span className="cmp-cap">{t("components.copy.443")}</span>
    </Block>
    <Block title={t("components.copy.444")} note={t("components.copy.445")} wide col>
      <div className={"banner " + (offline ? "warn" : "info")}><Icon name={offline ? "globe" : "check-circle"} /><span>{offline ? t("components.copy.446") : t("components.copy.447")}</span><span className="grow">{offline ? t("components.copy.448") : t("components.copy.449")}</span><button className="btn secondary" onClick={() => setOffline(!offline)}>{offline ? t("components.copy.450") : t("components.copy.451")}</button></div>
      <div className="banner warn"><Icon name="shield-check" /><span>{t("components.copy.452")}</span><span className="grow">{t("components.copy.453")}</span><button className="btn secondary" onClick={() => go("connectors")}>{t("components.copy.454")}</button></div>
      <div className="banner info"><Icon name="bolt" /><span>{t("components.copy.456")}</span><span className="grow">{t("components.copy.457")}</span><button className="btn secondary" onClick={() => go("pricing")}>{t("components.copy.458")}</button></div>
    </Block>
    <Block title={t("components.copy.459")} note={t("components.copy.460")} col>
      {empty ? <div className="empty"><Mascot cfg={bot} state="idle" size={56} /><h2>{t("components.copy.461")}</h2><p>{t("components.copy.462")}</p><button className="btn primary" onClick={() => setEmpty(false)}>{t("components.copy.463")}</button></div> : <div className="cmp-stack"><div className="li"><Icon name="folder" /><span className="ttl" role="status">{t("components.copy.059")}</span></div><button className="btn secondary" onClick={() => setEmpty(true)}>{t("components.copy.464")}</button></div>}
    </Block>
    <Block title={t("components.copy.465")} note={t("components.copy.466")} col>
      <div className="empty"><Mascot cfg={bot} state={error ? "blocked" : "done"} size={56} /><h2>{error ? t("components.copy.467") : t("components.copy.468")}</h2><p role="status">{error ? t("components.copy.469") : t("components.copy.470")}</p><button className="btn secondary" onClick={() => setError(!error)}><Icon name="refresh" />{error ? t("components.copy.471") : t("components.copy.472")}</button></div>
    </Block>
  </>);
}

function MotionBlocks() {
  const t = useT();
  const [q, setQ] = React.useState("");
  const source = (i: number) => i <= 1 ? ["styles.css", "shell"] : i === 2 ? [t("components.copy.473"), "navigation"] : i === 3 ? [t("components.copy.474"), "work"] : i <= 5 ? [t("components.copy.475"), "overlays"] : i <= 7 ? [t("components.copy.476"), "navigation"] : i <= 12 ? [t("components.copy.477"), "shell"] : i === 13 ? [t("components.copy.478"), "navigation"] : i === 14 ? [t("components.copy.479"), "forms"] : i <= 17 ? [t("components.copy.480"), "bots"] : i <= 19 ? [t("components.copy.481"), "bots"] : i <= 21 ? [t("components.copy.482"), "work"] : i <= 23 ? [t("components.copy.483"), "chat"] : i === 24 ? [t("components.copy.484"), "files"] : i === 25 ? [t("components.copy.485"), "system"] : i <= 27 ? [t("components.copy.486"), "overlays"] : [t("components.copy.487"), "shell"];
  const rows = MOTIONS.map((motion, i) => ({ motion: motion.map((key) => t(key)), source: source(i) })).filter(({ motion }) => motion.join(" ").toLocaleLowerCase(document.documentElement.lang).includes(q.toLocaleLowerCase(document.documentElement.lang)));
  return <Block title={t("components.copy.488")} note={t("components.copy.489", { value1: MOTIONS.length })} wide col>
    <label className="field">{t("components.copy.490")}<input className="input" value={q} onChange={(e) => setQ(e.target.value)} placeholder={t("components.copy.491")} /></label>
    <span className="cmp-cap">{t("components.copy.492")}</span>
    <div className="cmp-table-scroll" tabIndex={0} role="region" aria-label={t("components.copy.493")}><table className="cmp-motion-table"><caption className="systeme-sr">{t("components.copy.494")}</caption><thead><tr>{[t("components.copy.495"), t("components.copy.496"), t("components.copy.497"), t("components.copy.498"), t("components.copy.499")].map((h) => <th scope="col" key={h}>{h}</th>)}</tr></thead><tbody>{rows.map(({ motion: [name, behavior, duration, easing], source: [file, section] }) => <tr key={name}><th scope="row"><button className="systeme-textbtn" onClick={() => goSection(section)}>{name}</button></th><td>{behavior}</td><td className="mono">{duration}</td><td className="mono">{easing}</td><td className="mono">{file}</td></tr>)}</tbody></table></div>
    <span className="cmp-cap" role="status">{t("components.motion.count", { count: rows.length })}</span>
  </Block>;
}

function useDemoBot(): MascotConfig {
  const fx = useFixtures<{ main: { name: string } }>("bots");
  return { ...DEFAULT_MASCOT, name: fx.main.name };
}

function useCatalogNav() {
  const { go } = useNav();
  return { go: (id: string) => go(screenFor(id)?.id ?? id, { preview: "" }) };
}

const screenFor = (design: string) => SCREENS.find((screen) => (screen.design ?? screen.id) === design);
const variantFor = (design: string, variant?: string) => {
  const variants = screenFor(design)?.variants;
  return variants?.find(([id, , designID]) => id === variant || designID === variant)?.[0] ?? variants?.[0]?.[0] ?? "";
};

export function ComponentsScreen() {
  const t = useT();
  const { go } = useNav();
  const { locale } = useI18n();
  if (isPreview()) return <Catalog key={locale} />;
  return <div className="page"><div className="empty"><Icon name="projects" size={32} /><h1>{t("components.title")}</h1><p>{t("components.demoOnly")}</p><button className="btn primary" onClick={() => go("components", { preview: "" })}>{t("components.openCatalog")}</button></div></div>;
}
