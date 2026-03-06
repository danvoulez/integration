import { ProjectStatus } from "@/lib/data";

const cfg: Record<ProjectStatus, { label: string; dot: string }> = {
  active: { label: "Online", dot: "var(--green)" },
  paused: { label: "Offline", dot: "var(--red)" },
  archived: { label: "Arquivado", dot: "var(--t3)" },
  draft: { label: "Draft", dot: "var(--t3)" },
};

export default function StatusBadge({ status }: { status: ProjectStatus }) {
  const c = cfg[status];
  return (
    <span style={{
      display: "inline-flex", alignItems: "center", gap: 6,
      fontSize: 10, fontWeight: 700, letterSpacing: "0.08em",
      textTransform: "uppercase", color: "var(--t1)",
      padding: "4px 10px 4px 8px",
      border: "1px solid var(--line)",
      borderRadius: 6,
      background: "var(--bg-card)",
    }}>
      <span style={{
        width: 6, height: 6, borderRadius: "50%",
        background: c.dot, display: "inline-block", flexShrink: 0,
      }} />
      {c.label}
    </span>
  );
}
