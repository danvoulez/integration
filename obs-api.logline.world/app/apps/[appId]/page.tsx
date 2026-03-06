"use client";

import { use, useState } from "react";
import Link from "next/link";
import { notFound } from "next/navigation";
import AppShell from "@/components/AppShell";
import StatusBadge from "@/components/StatusBadge";
import { apps } from "@/lib/data";

const statusOrder = { active: 0, paused: 1, draft: 2, archived: 3 };

export default function AppDetailPage({ params }: { params: Promise<{ appId: string }> }) {
  const { appId } = use(params);
  const [filter, setFilter] = useState("all");

  const app = apps.find((a) => a.id === appId);
  if (!app) return notFound();

  const statusCounts = app.projects.reduce(
    (acc, p) => { acc[p.status] = (acc[p.status] || 0) + 1; return acc; },
    {} as Record<string, number>
  );

  const filters = [
    { key: "all", label: "Todos" },
    { key: "active", label: "Online" },
    { key: "paused", label: "Offline" },
    { key: "draft", label: "Draft" },
  ];

  const filtered = app.projects
    .filter((p) => filter === "all" || p.status === filter)
    .sort((a, b) => statusOrder[a.status] - statusOrder[b.status]);

  return (
    <AppShell showBack backHref="/">
      <main style={{ maxWidth: 640, margin: "0 auto", padding: "8px 20px 80px" }}>

        {/* Large title */}
        <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 4 }}>
          <div style={{
            width: 52, height: 52, borderRadius: 14, flexShrink: 0,
            display: "flex", alignItems: "center", justifyContent: "center",
            fontSize: 26, background: "linear-gradient(145deg, var(--white) 0%, var(--bg) 100%)",
            boxShadow: "0 2px 8px rgba(0,0,0,0.04), inset 0 1px 0 rgba(255,255,255,0.8)",
            border: "1px solid var(--line)",
          }}>
            {app.icon}
          </div>
          <div>
            <h1 style={{
              fontSize: 28, fontWeight: 800, letterSpacing: "-0.03em",
              color: "var(--t1)", margin: 0, lineHeight: 1.1,
            }}>
              {app.name}
            </h1>
            <p style={{ margin: "4px 0 0", fontSize: 13, color: "var(--t2)" }}>{app.subtitle}</p>
          </div>
        </div>

        {/* Status counts */}
        <div className="metric-card" style={{ display: "flex", gap: 24, justifyContent: "center", marginTop: 20, marginBottom: 24, padding: "16px 24px" }}>
          {[
            { label: "Online", count: statusCounts.active || 0, dot: "var(--green)" },
            { label: "Offline", count: (statusCounts.paused || 0) + (statusCounts.draft || 0), dot: "var(--red)" },
            { label: "Total", count: app.projects.length, dot: "var(--t3)" },
          ].map(({ label, count, dot }) => (
            <div key={label} style={{ textAlign: "center" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 6 }}>
                <span style={{ width: 8, height: 8, borderRadius: "50%", background: dot, display: "inline-block" }} />
                <span style={{ fontSize: 28, fontWeight: 800, color: "var(--t1)", letterSpacing: "-0.04em", lineHeight: 1 }}>{count}</span>
              </div>
              <p className="label" style={{ marginTop: 4 }}>{label}</p>
            </div>
          ))}
        </div>

        {/* Filter chips */}
        <div style={{ display: "flex", gap: 6, marginBottom: 16, overflowX: "auto", scrollbarWidth: "none" }}>
          {filters.map((f) => {
            const count = f.key === "all" ? app.projects.length : (statusCounts[f.key] || 0);
            if (f.key !== "all" && count === 0) return null;
            return (
              <button
                key={f.key}
                onClick={() => setFilter(f.key)}
                style={{
                  display: "inline-flex", alignItems: "center", gap: 6,
                  padding: "7px 14px", borderRadius: 8,
                  fontSize: 12, fontWeight: 600, letterSpacing: "0.02em",
                  border: "1px solid var(--line)",
                  background: filter === f.key ? "var(--black)" : "var(--bg-card)",
                  color: filter === f.key ? "var(--white)" : "var(--t2)",
                  cursor: "pointer", whiteSpace: "nowrap",
                  transition: "all 0.15s ease",
                }}
              >
                {f.key === "active" && <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--green)", display: "inline-block" }} />}
                {f.key === "paused" && <span style={{ width: 6, height: 6, borderRadius: "50%", background: "var(--red)", display: "inline-block" }} />}
                {f.label}
                <span style={{
                  fontSize: 10, fontWeight: 700,
                  background: filter === f.key ? "rgba(255,255,255,0.2)" : "var(--line)",
                  padding: "2px 6px", borderRadius: 4,
                }}>
                  {count}
                </span>
              </button>
            );
          })}
        </div>

        {/* Project list */}
        <div className="card">
          {filtered.length === 0 && (
            <div style={{ padding: "40px 16px", textAlign: "center", color: "var(--t3)", fontSize: 13 }}>
              Nenhum projeto
            </div>
          )}

          {filtered.map((project, idx) => {
            const isOnline = project.status === "active";
            const isError = project.status === "paused";

            return (
              <Link
                key={project.id}
                href={`/apps/${appId}/project/${project.id}`}
                className={`row row-enter d-${Math.min(idx, 7)}`}
                style={{ borderBottom: idx < filtered.length - 1 ? "1px solid var(--line)" : "none", alignItems: "flex-start" }}
              >
                <div style={{ paddingTop: 2, flexShrink: 0, display: "flex", flexDirection: "column", alignItems: "center", gap: 4 }}>
                  {isOnline ? (
                    <span className="dot-live" />
                  ) : (
                    <span className="dot" style={{ background: isError ? "var(--red)" : "var(--t3)" }} />
                  )}
                </div>

                <div style={{ flex: 1, minWidth: 0 }}>
                  <p style={{ margin: 0, fontSize: 15, fontWeight: 700, color: "var(--t1)", letterSpacing: "-0.01em" }}>
                    {project.name}
                  </p>
                  <p style={{ margin: "3px 0 8px", fontSize: 12, color: "var(--t2)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {project.description}
                  </p>
                  <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
                    {project.tech.slice(0, 3).map((t) => (
                      <span key={t} style={{
                        fontSize: 10, fontWeight: 600, letterSpacing: "0.04em",
                        textTransform: "uppercase",
                        padding: "3px 8px", borderRadius: 4,
                        background: "var(--bg)", color: "var(--t2)",
                        border: "1px solid var(--line)",
                      }}>
                        {t}
                      </span>
                    ))}
                  </div>
                </div>

                <div style={{ flexShrink: 0, paddingTop: 2 }}>
                  <StatusBadge status={project.status} />
                </div>
              </Link>
            );
          })}
        </div>

        <p style={{ textAlign: "center", fontSize: 10, color: "var(--t3)", marginTop: 12, letterSpacing: "0.08em" }}>
          {app.projects.length} PROJETOS — {app.subtitle}
        </p>
      </main>
    </AppShell>
  );
}
