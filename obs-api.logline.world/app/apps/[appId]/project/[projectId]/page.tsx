"use client";

import { use } from "react";
import { notFound } from "next/navigation";
import AppShell from "@/components/AppShell";
import { apps } from "@/lib/data";

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{
      display: "flex", justifyContent: "space-between", alignItems: "flex-start",
      padding: "12px 16px", borderBottom: "1px solid var(--line)",
    }}>
      <span style={{ fontSize: 12, color: "var(--t2)", flexShrink: 0 }}>{label}</span>
      <span style={{
        fontSize: 12, fontWeight: 700, textAlign: "right", marginLeft: 16,
        color: "var(--t1)", maxWidth: "64%", wordBreak: "break-all",
        fontFamily: mono ? "monospace" : "inherit",
        letterSpacing: mono ? "0.01em" : "inherit",
      }}>
        {value}
      </span>
    </div>
  );
}

export default function ProjectDetailPage({
  params,
}: {
  params: Promise<{ appId: string; projectId: string }>;
}) {
  const { appId, projectId } = use(params);
  const app = apps.find((a) => a.id === appId);
  if (!app) return notFound();
  const project = app.projects.find((p) => p.id === projectId);
  if (!project) return notFound();

  const isOnline = project.status === "active";
  const isError = project.status === "paused";
  const dotColor = isOnline ? "var(--green)" : isError ? "var(--red)" : "var(--t3)";
  const dotLabel = isOnline ? "Online" : isError ? "Offline" : "Draft";
  const statusLabel = { active: "Ativo", paused: "Pausado", draft: "Rascunho", archived: "Arquivado" }[project.status];

  return (
    <AppShell showBack backHref={`/apps/${appId}`}>
      <main style={{ maxWidth: 640, margin: "0 auto", padding: "8px 20px 80px" }}>

        {/* ── Hero ── */}
        <div
          className="metric-card"
          style={{
            marginBottom: 16, overflow: "hidden", padding: 0,
            borderTop: "3px solid var(--black)",
          }}
        >
          {/* Top strip with signal */}
          <div style={{
            display: "flex", alignItems: "center", justifyContent: "space-between",
            padding: "14px 20px 0",
          }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              {isOnline ? (
                <span className="dot-live" />
              ) : (
                <span className="dot" style={{ background: dotColor }} />
              )}
              <span style={{
                fontSize: 10, fontWeight: 700, letterSpacing: "0.1em",
                textTransform: "uppercase", color: dotColor,
              }}>
                {dotLabel}
              </span>
            </div>
            <span style={{
              fontSize: 10, fontWeight: 700, letterSpacing: "0.06em",
              textTransform: "uppercase", color: "var(--t3)",
            }}>
              {app.name}
            </span>
          </div>

          {/* Main content */}
          <div style={{ padding: "12px 20px 20px" }}>
            <h1 style={{
              margin: 0, fontSize: 28, fontWeight: 800,
              letterSpacing: "-0.03em", color: "var(--t1)", lineHeight: 1.15,
            }}>
              {project.name}
            </h1>
            {project.description && (
              <p style={{ margin: "8px 0 0", fontSize: 14, color: "var(--t2)", lineHeight: 1.6 }}>
                {project.description}
              </p>
            )}
          </div>

          {/* Bottom metadata strip */}
          <div style={{
            display: "flex", alignItems: "center", gap: 16,
            padding: "12px 20px",
            borderTop: "1px solid var(--line)",
            background: "var(--bg)",
          }}>
            <div style={{ flex: 1 }}>
              <p className="label" style={{ marginBottom: 2 }}>Status</p>
              <p style={{ margin: 0, fontSize: 12, fontWeight: 700, color: "var(--t1)" }}>{statusLabel}</p>
            </div>
            <div style={{ flex: 1 }}>
              <p className="label" style={{ marginBottom: 2 }}>Atualizado</p>
              <p style={{ margin: 0, fontSize: 12, fontWeight: 700, color: "var(--t1)" }}>{project.updatedAt}</p>
            </div>
            <div style={{ flex: 1, display: "flex", justifyContent: "flex-end" }}>
              <div style={{
                width: 42, height: 42, borderRadius: 10,
                display: "flex", alignItems: "center", justifyContent: "center",
                fontSize: 20, background: "linear-gradient(145deg, var(--white) 0%, var(--bg) 100%)",
                border: "1px solid var(--line)",
                boxShadow: "0 2px 8px rgba(0,0,0,0.04), inset 0 1px 0 rgba(255,255,255,0.8)",
              }}>
                {app.icon}
              </div>
            </div>
          </div>
        </div>

        {/* ── Details ── */}
        <div className="card" style={{ marginBottom: 12 }}>
          <div style={{ padding: "10px 16px 6px", borderBottom: "1px solid var(--line)" }}>
            <p className="label">Detalhes</p>
          </div>
          {project.repo && <Row label="Repositório" value={project.repo} mono />}
          {project.url && <Row label="URL" value={project.url} mono />}
          {!project.repo && !project.url && (
            <div style={{ padding: "12px 16px" }}>
              <p style={{ margin: 0, fontSize: 12, color: "var(--t3)" }}>Sem repositório ou URL definidos.</p>
            </div>
          )}
          <div style={{ height: 1 }} />
        </div>

        {/* ── Stack ── */}
        <div className="card" style={{ marginBottom: 12 }}>
          <div style={{ padding: "10px 16px", borderBottom: "1px solid var(--line)" }}>
            <p className="label">Stack</p>
          </div>
          <div style={{ padding: "12px 16px", display: "flex", flexWrap: "wrap", gap: 6 }}>
            {project.tech.map((t) => (
              <span
                key={t}
                style={{
                  display: "inline-flex", alignItems: "center",
                  padding: "7px 14px", fontSize: 12, fontWeight: 600,
                  borderRadius: 8, border: "1px solid var(--line)",
                  background: "var(--bg-card)", color: "var(--t1)",
                }}
              >
                {t}
              </span>
            ))}
          </div>
        </div>

        {/* ── Notes ── */}
        {project.notes && (
          <div className="card" style={{ marginBottom: 12 }}>
            <div style={{ padding: "10px 16px", borderBottom: "1px solid var(--line)" }}>
              <p className="label">Notas</p>
            </div>
            <div style={{ padding: "14px 16px" }}>
              <p style={{ margin: 0, fontSize: 13, color: "var(--t2)", lineHeight: 1.65 }}>
                {project.notes}
              </p>
            </div>
          </div>
        )}

        {/* ── Actions ── */}
        <div style={{ display: "flex", flexDirection: "column", gap: 8, marginBottom: 12 }}>
          <button className="btn" style={{ width: "100%", borderRadius: 8, gap: 10 }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
              <path d="M12 20h9M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            Editar Projeto
          </button>
        </div>

        <p style={{ textAlign: "center", fontSize: 10, color: "var(--t3)", paddingBottom: 16, letterSpacing: "0.08em" }}>
          {project.name} — {project.updatedAt}
        </p>
      </main>
    </AppShell>
  );
}
