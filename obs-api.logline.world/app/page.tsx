"use client";

import { useState } from "react";
import AppShell from "@/components/AppShell";
import { fuelByApps, fuelByTenants, fuelByUsers, totalFuel, FuelMetric } from "@/lib/data";

type ViewMode = "realtime" | "estatisticas";

function ViewToggle({ mode, onChange }: { mode: ViewMode; onChange: (m: ViewMode) => void }) {
  return (
    <div style={{
      display: "inline-flex",
      background: "var(--bg2)",
      borderRadius: 8,
      padding: 3,
      marginBottom: 28,
    }}>
      <button
        onClick={() => onChange("realtime")}
        style={{
          padding: "8px 18px",
          fontSize: 13,
          fontWeight: 600,
          borderRadius: 6,
          border: "none",
          cursor: "pointer",
          transition: "all 0.15s ease",
          background: mode === "realtime" ? "var(--bg1)" : "transparent",
          color: mode === "realtime" ? "var(--t1)" : "var(--t3)",
          boxShadow: mode === "realtime" ? "0 1px 3px rgba(0,0,0,0.1)" : "none",
        }}
      >
        Realtime
      </button>
      <button
        onClick={() => onChange("estatisticas")}
        style={{
          padding: "8px 18px",
          fontSize: 13,
          fontWeight: 600,
          borderRadius: 6,
          border: "none",
          cursor: "pointer",
          transition: "all 0.15s ease",
          background: mode === "estatisticas" ? "var(--bg1)" : "transparent",
          color: mode === "estatisticas" ? "var(--t1)" : "var(--t3)",
          boxShadow: mode === "estatisticas" ? "0 1px 3px rgba(0,0,0,0.1)" : "none",
        }}
      >
        Estatísticas
      </button>
    </div>
  );
}

function fuelLevel(pct: number) {
  if (pct >= 80) return "fuel-critical";
  if (pct >= 60) return "fuel-warning";
  return "fuel-ok";
}

function formatNum(n: number) {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n);
}

function TrendArrow({ trend }: { trend: number }) {
  if (trend === 0) return <span className="trend-flat" style={{ fontSize: 11, fontWeight: 600 }}>—</span>;
  const isUp = trend > 0;
  return (
    <span className={isUp ? "trend-up" : "trend-down"} style={{ fontSize: 11, fontWeight: 700, display: "inline-flex", alignItems: "center", gap: 2 }}>
      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" style={{ transform: isUp ? "none" : "rotate(180deg)" }}>
        <path d="M12 19V5M5 12l7-7 7 7" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
      {Math.abs(trend)}%
    </span>
  );
}

function FuelSection({ title, data }: { title: string; data: FuelMetric[] }) {
  return (
    <div style={{ marginBottom: 28 }}>
      <p className="label" style={{ marginBottom: 12, paddingLeft: 2 }}>{title}</p>
      <div className="fuel-grid">
        {data.map((m, idx) => {
          const pct = Math.round((m.value / m.max) * 100);
          return (
            <div key={m.id} className={`metric-card row-enter d-${Math.min(idx, 7)}`}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                <span style={{ fontSize: 14, fontWeight: 700, color: "var(--t1)" }}>{m.label}</span>
                <TrendArrow trend={m.trend} />
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 8 }}>
                <span style={{ fontSize: 24, fontWeight: 800, letterSpacing: "-0.03em", color: "var(--t1)", lineHeight: 1 }}>
                  {formatNum(m.value)}
                </span>
                <span style={{ fontSize: 11, color: "var(--t3)", fontWeight: 500 }}>
                  / {formatNum(m.max)} {m.unit}
                </span>
              </div>
              <div className="fuel-bar-track">
                <div
                  className={`fuel-bar-fill ${fuelLevel(pct)}`}
                  style={{ width: `${pct}%` }}
                />
              </div>
              <p style={{ margin: "6px 0 0", fontSize: 11, color: "var(--t3)", textAlign: "right" }}>{pct}%</p>
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default function EcossistemaPage() {
  const [viewMode, setViewMode] = useState<ViewMode>("realtime");
  const totalPct = Math.round((totalFuel.consumed / totalFuel.budget) * 100);

  return (
    <AppShell>
      <main style={{ maxWidth: 640, margin: "0 auto", padding: "8px 20px 80px" }}>

        {/* Large page title */}
        <h1 style={{
          fontSize: 32, fontWeight: 800, letterSpacing: "-0.03em",
          color: "var(--t1)", margin: "8px 0 4px", lineHeight: 1.1,
        }}>
          Ecossistema
        </h1>
        <p style={{ fontSize: 13, color: "var(--t2)", margin: "0 0 20px", lineHeight: 1.5 }}>
          Fuel consumption — recursos consumidos em tempo real.
        </p>

        {/* View Toggle */}
        <ViewToggle mode={viewMode} onChange={setViewMode} />

        {/* ── Total gauge ── */}
        <div className="metric-card" style={{ marginBottom: 32, textAlign: "center", padding: "28px 24px" }}>
          <p className="label" style={{ marginBottom: 8 }}>Total Consumo</p>
          <div style={{ display: "flex", alignItems: "baseline", justifyContent: "center", gap: 8, marginBottom: 12 }}>
            <span style={{ fontSize: 48, fontWeight: 800, letterSpacing: "-0.04em", color: "var(--t1)", lineHeight: 1 }}>
              {formatNum(totalFuel.consumed)}
            </span>
            <span style={{ fontSize: 14, color: "var(--t3)", fontWeight: 500 }}>
              / {formatNum(totalFuel.budget)} {totalFuel.unit}
            </span>
          </div>
          <div className="fuel-bar-track" style={{ height: 10, borderRadius: 5, maxWidth: 400, margin: "0 auto" }}>
            <div
              className={`fuel-bar-fill ${fuelLevel(totalPct)}`}
              style={{ width: `${totalPct}%`, borderRadius: 5 }}
            />
          </div>
          <p style={{ margin: "10px 0 0", fontSize: 12, color: "var(--t3)" }}>{totalPct}% da capacidade</p>
        </div>

        {/* ── Sections ── */}
        <FuelSection title="Por Apps" data={fuelByApps} />
        <FuelSection title="Por Tenants" data={fuelByTenants} />
        <FuelSection title="Por Users" data={fuelByUsers} />

        <p style={{ textAlign: "center", fontSize: 10, color: "var(--t3)", letterSpacing: "0.08em", marginTop: 12 }}>
          UBL WORKSPACE — FUEL CONSUMPTION
        </p>
      </main>
    </AppShell>
  );
}
