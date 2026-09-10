import { FormEvent, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

type Order = { id: number; customer: string; amount: number };
type ServiceState = { name: string; language: string; status: string; database?: string };

const money = new Intl.NumberFormat("en-US", { style: "currency", currency: "IDR", maximumFractionDigits: 0 });

async function readJson<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, options);
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  return response.json() as Promise<T>;
}

function App() {
  const [orders, setOrders] = useState<Order[]>([]);
  const [services, setServices] = useState<ServiceState[]>([]);
  const [customer, setCustomer] = useState("BukuWarung");
  const [amount, setAmount] = useState("125000");
  const [message, setMessage] = useState("Ready. PostgreSQL-backed orders are waiting for work.");
  const [busy, setBusy] = useState(false);

  const refresh = async () => {
    try {
      const [orderRows, orderHealth, userHealth] = await Promise.all([
        readJson<Order[]>("/orders"),
        readJson<ServiceState>("/orders/health"),
        readJson<ServiceState>("/users/health"),
      ]);
      setOrders(orderRows);
      setServices([orderHealth, userHealth]);
      setMessage(`${orderRows.length} recent order${orderRows.length === 1 ? "" : "s"} loaded from PostgreSQL.`);
    } catch (error) {
      setMessage(`API unavailable: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  useEffect(() => { void refresh(); }, []);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    try {
      const order = await readJson<Order>("/orders", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ customer, amount: Number(amount) }),
      });
      setMessage(`Order #${order.id} persisted for ${order.customer}.`);
      await refresh();
    } catch (error) {
      setMessage(`Could not persist order: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><span className="mark">P</span><div><strong>PITFAST</strong><span>COMMERCE DEMO</span></div></div>
      <div className="status"><span className="status-dot" /> deployed · disposable execution</div>
    </header>
    <main>
      <section className="hero">
        <div>
          <p className="eyebrow">EXECUTION-FIRST COMMERCE</p>
          <h1>Deployed does not mean running.</h1>
          <p className="lede">Orders, users, and the web experience share PitFast execution capacity. Order truth lives in external PostgreSQL; guest work is created only when a request arrives.</p>
        </div>
        <div className="hero-card"><span>STORAGE</span><strong>PostgreSQL</strong><small>external · durable</small></div>
      </section>
      <section className="service-grid">{services.map((service) => <article className="service-card" key={service.name}><div className="service-head"><span className="service-light" />{service.name}</div><strong>{service.language}</strong><small>{service.status} · {service.database ?? "logical service"}</small></article>)}</section>
      <section className="content-grid">
        <article className="panel order-panel">
          <div className="panel-heading"><div><p className="eyebrow">REAL BUSINESS FLOW</p><h2>Create an order</h2></div><span className="chip">POSTGRESQL</span></div>
          <form onSubmit={submit}>
            <label>Customer<input value={customer} onChange={(event) => setCustomer(event.target.value)} required /></label>
            <label>Amount (IDR)<input inputMode="numeric" type="number" min="1" value={amount} onChange={(event) => setAmount(event.target.value)} required /></label>
            <button disabled={busy} type="submit">{busy ? "Persisting…" : "Persist order"}</button>
          </form>
          <p className="message" role="status">{message}</p>
        </article>
        <article className="panel">
          <div className="panel-heading"><div><p className="eyebrow">DURABLE STATE</p><h2>Recent orders</h2></div><button className="secondary" onClick={() => void refresh()}>Refresh</button></div>
          {orders.length === 0 ? <p className="empty">No orders yet. Create the first one.</p> : <div className="orders">{orders.map((order) => <div className="order-row" key={order.id}><span className="order-id">#{order.id}</span><span>{order.customer}</span><strong>{money.format(order.amount)}</strong></div>)}</div>}
        </article>
      </section>
      <section className="explain"><span className="explain-number">01</span><div><h2>Watch it run</h2><p>Open <code>pit cockpit</code>, then use the buttons or burst script. The same orders service is routed through one PitLane listener and assigned a shared Lane only for the duration of each request.</p></div><a href="/orders/health">Orders health ↗</a></section>
    </main>
    <footer>web · React/Vite static-web &nbsp;·&nbsp; orders · Go net/http + pgx &nbsp;·&nbsp; users · Python ASGI</footer>
  </div>;
}

createRoot(document.getElementById("root")!).render(<App />);
