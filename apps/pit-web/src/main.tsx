import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

type Snapshot = {
  system: { version: string; os: string; architecture: string; kernel?: string; logical_cpus?: number; running_executions: number };
  applications: Application[];
  releases: Release[];
  routes: Route[];
  services: Array<{ service_id: string; active: boolean; generation: number; artifact_digest?: string; status: string; prepared: boolean }>;
  paddock: { artifact_store: { backend: string; artifact_count: number; artifacts: Artifact[] }; object_backend?: { backend: string; capabilities: Record<string, boolean> }; namespaces: string[]; objects: ObjectEntry[] };
  circuit?: { circuit_id: string; version: number; garages: Garage[] };
};
type Application = { application_id: string; active_release_id?: string; manifest_digest?: string; status: string; services: AppService[]; routes: Array<{ path: string; service_id: string }> };
type AppService = { service_id: string; artifact_digest: string; interface?: string };
type Release = { application_id: string; release_id: string; manifest_digest: string; created_at_unix_ms: number; active: boolean; services: AppService[]; routes: Array<{ path: string; service_id: string }> };
type Route = { application_id: string; path: string; service_id: string; release_id: string; host?: string; stable_release?: string; candidate_release?: string; strategy?: string; weights?: string; stickiness?: string; shadow?: boolean; snapshot_generation?: number };
type Artifact = { digest: string; name: string; size_bytes: number; interface?: string };
type ObjectEntry = { namespace: string; key: string; version: number; digest: string; size_bytes: number; content_type?: string; deleted: boolean };
type Garage = { id: string; endpoint: string; total_lanes: number; active_lanes: number; free_lanes: number; health: string; last_heartbeat_unix_ms: number; capabilities: { architecture: string; lane_capacity: number } };

const pages = ["Overview", "Applications", "Releases", "Routes", "Paddock", "Circuit", "System"] as const;
type Page = typeof pages[number];

function apiUrl() {
  return `${window.location.protocol}//${window.location.hostname}:7081/v1/management/snapshot`;
}
function short(value: string | undefined) { return value ? `${value.slice(0, 16)}…` : "—"; }
function bytes(value: number) { return value < 1024 ? `${value} B` : `${(value / 1024 / 1024).toFixed(2)} MiB`; }
function time(value: number | undefined) { return value ? new Date(value).toLocaleString() : "—"; }

function App() {
  const [page, setPage] = useState<Page>("Overview");
  const [data, setData] = useState<Snapshot>();
  const [error, setError] = useState<string>();
  useEffect(() => {
    let mounted = true;
    const load = () => fetch(apiUrl()).then((response) => response.ok ? response.json() : Promise.reject(new Error(`${response.status} ${response.statusText}`)))
      .then((value) => { if (mounted) { setData(value); setError(undefined); } })
      .catch((reason: Error) => { if (mounted) setError(`Management API unavailable: ${reason.message}`); });
    load();
    const timer = window.setInterval(load, 2000);
    return () => { mounted = false; window.clearInterval(timer); };
  }, []);
  return <div className="shell">
    <aside><div className="brand"><span className="flag">◆</span><span>PITFAST<br /><small>LOCAL WEB</small></span></div><p className="eyebrow">MANAGEMENT / READ ONLY</p><nav>{pages.map((item) => <button className={page === item ? "selected" : ""} key={item} onClick={() => setPage(item)}>{item}</button>)}</nav><div className="aside-note">Pit Web manages durable state.<br />Cockpit watches execution.</div></aside>
    <main><header><div><p className="eyebrow">PITFAST LOCAL</p><h1>{page}</h1></div><div className="status"><span className="dot" /> authority connected<br /><small>control API · local listener</small></div></header>
      {error && <div className="error">{error}</div>}
      {!data && !error && <div className="empty">Waiting for the local PitLane management snapshot…</div>}
      {data && <Content page={page} data={data} setPage={setPage} />}
    </main>
  </div>;
}

function Content({ page, data, setPage }: { page: Page; data: Snapshot; setPage: (page: Page) => void }) {
  if (page === "Overview") return <Overview data={data} setPage={setPage} />;
  if (page === "Applications") return <Applications data={data} />;
  if (page === "Releases") return <Releases data={data} />;
  if (page === "Routes") return <Routes data={data} />;
  if (page === "Paddock") return <Paddock data={data} />;
  if (page === "Circuit") return <Circuit data={data} />;
  return <System data={data} />;
}

function Overview({ data, setPage }: { data: Snapshot; setPage: (page: Page) => void }) {
  const active = data.applications.filter((app) => app.status === "active").length;
  return <><section className="hero"><div><p className="eyebrow">EXECUTION-FIRST PLATFORM</p><h2>Persistent authority.<br /><em>Disposable execution.</em></h2><p className="lede">Pit Web shows what exists and what is deployed. Open Cockpit when you want to watch work move through shared lanes.</p></div><a className="button" href="https://github.com/pitfast/pit-cli/blob/master/docs/cockpit-demo.md" target="_blank">Cockpit demo guide ↗</a></section><div className="cards">{[["Applications", active, "apps"], ["Services", data.services.length, "logical services"], ["Active releases", data.releases.filter((r) => r.active).length, "release authority"], ["Artifacts", data.paddock.artifact_store.artifact_count, "immutable wasm"]].map(([label, value, caption]) => <div className="card" key={String(label)}><span>{label}</span><strong>{value}</strong><small>{caption}</small></div>)}</div><section className="split"><Panel title="PitLane status"><dl><dt>Listener</dt><dd>local / logical routes</dd><dt>Running executions</dt><dd className="accent">{data.system.running_executions}</dd><dt>Service-owned ports</dt><dd>0</dd><dt>Replica model</dt><dd>none</dd></dl></Panel><Panel title="Active applications"><Table headers={["Application", "Release", "Services"]}>{data.applications.map((app) => <tr key={app.application_id}><td>{app.application_id}</td><td className="mono">{short(app.active_release_id)}</td><td>{app.services.length}</td></tr>)}</Table>{data.applications.length === 0 && <Empty text="No applications deployed. Use pit up." />}</Panel></section></>;
}
function Applications({ data }: { data: Snapshot }) { return <PageIntro title="Applications" copy="Application identity and active ApplicationRelease state from PitLane." panels={data.applications.map((app) => <Panel title={app.application_id} key={app.application_id}><div className="facts"><span>status <b className="pill">{app.status}</b></span><span>active release <b className="mono">{short(app.active_release_id)}</b></span><span>manifest <b className="mono">{short(app.manifest_digest)}</b></span></div><Table headers={["Service", "Interface", "Artifact"]}>{app.services.map((service) => <tr key={service.service_id}><td>{service.service_id}</td><td>{service.interface ?? "unknown"}</td><td className="mono">{short(service.artifact_digest)}</td></tr>)}</Table><h4>Routes</h4><RouteTable routes={app.routes.map((r) => ({ ...r, application_id: app.application_id, release_id: app.active_release_id ?? "—" }))} /></Panel>)} empty="No applications deployed. Use pit up." />; }
function Releases({ data }: { data: Snapshot }) { return <PageIntro title="Releases" copy="Immutable application generations. ACTIVE is emphasized; mutation stays in the CLI." panels={[<Panel title="Release history" key="releases"><Table headers={["State", "Application", "Release", "Manifest", "Created"]}>{data.releases.map((release) => <tr key={`${release.application_id}-${release.release_id}`}><td><span className={release.active ? "pill active" : "pill"}>{release.active ? "ACTIVE" : "previous"}</span></td><td>{release.application_id}</td><td className="mono">{release.release_id}</td><td className="mono">{short(release.manifest_digest)}</td><td>{time(release.created_at_unix_ms)}</td></tr>)}</Table></Panel>]} empty="No release history yet." />; }
function Routes({ data }: { data: Snapshot }) { return <PageIntro title="Routes / PitLane" copy="Logical host and path selectors resolve to a ServiceId and coherent ApplicationRelease. Services do not own listeners." panels={[<Panel title={`Logical routing · snapshot ${data.routes[0]?.snapshot_generation ?? 0}`} key="routes"><RouteTable routes={data.routes} /></Panel>]} empty="No active routes." />; }
function RouteTable({ routes }: { routes: Array<Route> }) { return <Table headers={["Application", "Host / Path", "ServiceId", "Strategy", "Stable", "Candidate", "Traffic"]}>{routes.map((route) => <tr key={`${route.application_id}-${route.host ?? "*"}-${route.path}`}><td>{route.application_id}</td><td className="mono">{route.host ?? "*"} {route.path}</td><td className="accent">→ {route.service_id}</td><td><span className="pill">{route.strategy ?? "stable"}</span>{route.shadow ? <span className="pill active"> shadow</span> : null}</td><td className="mono">{short(route.stable_release ?? route.release_id)}</td><td className="mono">{short(route.candidate_release)}</td><td>{route.weights ?? "100/0"}{route.stickiness ? ` · ${route.stickiness}` : ""}</td></tr>)}</Table>; }
function Paddock({ data }: { data: Snapshot }) { const p = data.paddock; return <PageIntro title="Paddock" copy="Paddock stores truth: immutable artifact bytes and, when configured, durable object state." panels={[<Panel title={`Artifacts · ${p.artifact_store.backend}`} key="a"><Table headers={["Name", "Digest", "Size", "Interface"]}>{p.artifact_store.artifacts.map((a) => <tr key={a.digest}><td>{a.name}</td><td className="mono">{short(a.digest)}</td><td>{bytes(a.size_bytes)}</td><td>{a.interface ?? "—"}</td></tr>)}</Table></Panel>, <Panel title="Objects / namespaces" key="o"><div className="facts"><span>namespaces <b>{p.namespaces.length}</b></span><span>objects <b>{p.objects.length}</b></span><span>bytes are immutable · names may move</span></div>{p.objects.length ? <Table headers={["Namespace", "Key", "Version", "Digest"]}>{p.objects.map((o) => <tr key={`${o.namespace}/${o.key}`}><td>{o.namespace}</td><td className="mono">{o.key}</td><td>{o.version}</td><td className="mono">{short(o.digest)}</td></tr>)}</Table> : <Empty text={p.object_backend ? "No objects visible to the granted read/list namespaces." : "No object backend is attached to this local PitLane."} />}</Panel>, <Panel title="Backend capabilities" key="b">{p.object_backend ? <Table headers={["Backend", "Capability"]}>{Object.entries(p.object_backend.capabilities).map(([name, value]) => <tr key={name}><td>{name}</td><td className={value ? "good" : "muted"}>{value ? "supported" : "not advertised"}</td></tr>)}</Table> : <Empty text="Filesystem artifact store active; object capability not configured." />}</Panel>]} />; }
function Circuit({ data }: { data: Snapshot }) { return <PageIntro title="Circuit / Garages" copy="Topology and capacity inventory. Placement remains owned by Circuit." panels={[<Panel title={data.circuit ? `Circuit ${data.circuit.circuit_id}` : "Circuit unavailable"} key="c">{data.circuit ? <Table headers={["Garage", "Health", "Architecture", "Lanes", "Heartbeat"]}>{data.circuit.garages.map((g) => <tr key={g.id}><td>{g.id}</td><td><span className="pill">{g.health}</span></td><td>{g.capabilities.architecture}</td><td>{g.active_lanes}/{g.total_lanes}</td><td>{time(g.last_heartbeat_unix_ms)}</td></tr>)}</Table> : <Empty text="No Circuit client is configured for this local runtime." />}</Panel>]} />; }
function System({ data }: { data: Snapshot }) { return <PageIntro title="System" copy="Safe local runtime facts. Credentials and raw environment values are never returned." panels={[<Panel title="Runtime" key="runtime"><dl><dt>PitFast / PitLane</dt><dd>{data.system.version}</dd><dt>OS</dt><dd>{data.system.os}</dd><dt>Architecture</dt><dd>{data.system.architecture}</dd><dt>Kernel</dt><dd>{data.system.kernel ?? "N/A"}</dd><dt>Logical CPUs</dt><dd>{data.system.logical_cpus ?? "N/A"}</dd><dt>Control listener</dt><dd>local-configured</dd><dt>Running executions</dt><dd>{data.system.running_executions}</dd></dl></Panel>]} />; }
function PageIntro({ title, copy, panels, empty }: { title: string; copy: string; panels: React.ReactNode[]; empty?: string }) { return <><div className="intro"><p>{copy}</p></div>{panels.length ? <div className="stack">{panels}</div> : <Empty text={empty ?? "Nothing to show."} />}</>; }
function Panel({ title, children }: { title: string; children: React.ReactNode }) { return <section className="panel"><div className="panel-title"><span>{title}</span><i /></div>{children}</section>; }
function Table({ headers, children }: { headers: string[]; children: React.ReactNode }) { return <div className="table-wrap"><table><thead><tr>{headers.map((header) => <th key={header}>{header}</th>)}</tr></thead><tbody>{children}</tbody></table></div>; }
function Empty({ text }: { text: string }) { return <div className="empty small">{text}</div>; }

createRoot(document.getElementById("root")!).render(<App />);
