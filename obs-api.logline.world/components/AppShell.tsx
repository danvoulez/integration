"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useState, useEffect, ReactNode } from "react";
import { apps } from "@/lib/data";
import { useTheme } from "@/lib/theme";

interface AppShellProps {
    children: ReactNode;
    showBack?: boolean;
    backHref?: string;
}

export default function AppShell({ children, showBack, backHref }: AppShellProps) {
    const [sidebarOpen, setSidebarOpen] = useState(true);
    const pathname = usePathname();
    const router = useRouter();
    const { theme, toggle: toggleTheme } = useTheme();

    const isHome = pathname === "/" || pathname === "";

    // Default open on desktop, closed on mobile
    useEffect(() => {
        const mq = window.matchMedia("(min-width: 768px)");
        setSidebarOpen(mq.matches);
        const handler = (e: MediaQueryListEvent) => {
            if (e.matches) setSidebarOpen(true);
        };
        mq.addEventListener("change", handler);
        return () => mq.removeEventListener("change", handler);
    }, []);

    // Lock body scroll on mobile when sidebar open
    useEffect(() => {
        const isMobile = window.innerWidth < 768;
        if (isMobile && sidebarOpen) {
            document.body.style.overflow = "hidden";
        } else {
            document.body.style.overflow = "";
        }
        return () => { document.body.style.overflow = ""; };
    }, [sidebarOpen]);

    return (
        <div className="app-shell">
            {/* ── Mobile backdrop (no blur, just dim) ── */}
            <div
                className="sidebar-backdrop"
                onClick={() => setSidebarOpen(false)}
                style={{ opacity: sidebarOpen ? 1 : 0, pointerEvents: sidebarOpen ? "auto" : "none" }}
            />

            {/* ── Sidebar ── */}
            <aside
                className={`sidebar ${sidebarOpen ? "sidebar-open" : ""}`}
            >
                {/* Header */}
                <div style={{ padding: "32px 24px 20px" }}>
                    <p style={{ margin: 0, fontSize: 10, fontWeight: 700, letterSpacing: "0.16em", textTransform: "uppercase", color: "var(--t3)", marginBottom: 8 }}>
                        UBL System
                    </p>
                    <p style={{ margin: 0, fontSize: 22, fontWeight: 800, letterSpacing: "-0.03em", color: "var(--t1)", lineHeight: 1 }}>
                        Workspace
                    </p>
                </div>

                {/* Ecossistema nav */}
                <div style={{ padding: "0 12px" }}>
                    <Link
                        href="/"
                        onClick={() => { if (window.innerWidth < 768) setSidebarOpen(false); }}
                        className={isHome ? "" : "sidebar-nav-item"}
                        style={{
                            display: "flex", alignItems: "center", gap: 10,
                            padding: "10px 12px", borderRadius: 8,
                            background: isHome ? "var(--bg-card)" : "transparent",
                            color: isHome ? "var(--t1)" : "var(--t2)",
                            fontWeight: isHome ? 700 : 500,
                            fontSize: 13, textDecoration: "none",
                            boxShadow: isHome ? "var(--s1)" : "none",
                            border: isHome ? "1px solid var(--line)" : "1px solid transparent",
                            transition: "all 0.2s ease",
                        }}
                    >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                            <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="1.8" />
                            <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" strokeWidth="1.8" />
                            <path d="M2 12h20" stroke="currentColor" strokeWidth="1.8" />
                        </svg>
                        Ecossistema
                        {isHome && <div style={{ marginLeft: "auto", width: 6, height: 6, borderRadius: "50%", background: "var(--green)" }} />}
                    </Link>
                </div>

                {/* ── Divider ── */}
                <div style={{ margin: "16px 20px", height: 1, background: "var(--line)" }} />

                {/* ── App list ── */}
                <div style={{ padding: "0 12px", flex: 1, overflowY: "auto" }}>
                    <p className="label" style={{ padding: "0 12px 8px", margin: 0 }}>Apps</p>
                    {apps.map((app) => {
                        const activeCount = app.projects.filter((p) => p.status === "active").length;
                        const isOnline = activeCount > 0;
                        const isActive = pathname.startsWith(`/apps/${app.id}`);

                        return (
                            <Link
                                key={app.id}
                                href={`/apps/${app.id}`}
                                onClick={() => { if (window.innerWidth < 768) setSidebarOpen(false); }}
                                className={isActive ? "" : "sidebar-nav-item"}
                                style={{
                                    display: "flex", alignItems: "center", gap: 10,
                                    padding: "9px 12px", borderRadius: 8,
                                    background: isActive ? "var(--bg-card)" : "transparent",
                                    color: isActive ? "var(--t1)" : "var(--t2)",
                                    fontWeight: isActive ? 700 : 500,
                                    fontSize: 13, textDecoration: "none",
                                    boxShadow: isActive ? "var(--s1)" : "none",
                                    border: isActive ? "1px solid var(--line)" : "1px solid transparent",
                                    marginBottom: 2,
                                    transition: "all 0.15s ease",
                                }}
                            >
                                <span style={{ fontSize: 16 }}>{app.icon}</span>
                                <span style={{ flex: 1 }}>{app.name}</span>
                                <span
                                    className={isOnline ? "dot-live" : "dot dot-red"}
                                    title={isOnline ? `${activeCount} ativo(s)` : "Nenhum ativo"}
                                />
                            </Link>
                        );
                    })}
                </div>

                {/* Footer */}
                <div style={{ padding: "16px 24px 28px", borderTop: "1px solid var(--line)" }}>
                    <p style={{ margin: 0, fontSize: 10, color: "var(--t3)", letterSpacing: "0.08em" }}>v0.1.0 — UBL Workspace</p>
                </div>
            </aside>

            {/* ── Main content area ── */}
            <div className={`main-content ${sidebarOpen ? "main-content-shifted" : ""}`}>
                {/* Minimal top bar: hamburger + optional back */}
                <header className="content-header glass-header">
                    <div style={{ display: "flex", alignItems: "center", height: 48, padding: "0 8px", gap: 4 }}>
                        {showBack ? (
                            <button
                                onClick={() => backHref ? router.push(backHref) : router.back()}
                                className="press"
                                style={{
                                    width: 40, height: 40, borderRadius: 8, border: "none",
                                    background: "transparent", display: "flex", alignItems: "center",
                                    justifyContent: "center", cursor: "pointer", color: "var(--t3)", flexShrink: 0,
                                }}
                                aria-label="Voltar"
                            >
                                <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
                                    <path d="M19 12H5M5 12l7-7M5 12l7 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                                </svg>
                            </button>
                        ) : (
                            <button
                                onClick={() => setSidebarOpen(!sidebarOpen)}
                                className="press"
                                style={{
                                    width: 40, height: 40, borderRadius: 8, border: "none",
                                    background: "transparent", display: "flex", alignItems: "center",
                                    justifyContent: "center", cursor: "pointer", color: "var(--t3)", flexShrink: 0,
                                }}
                                aria-label="Menu"
                            >
                                <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
                                    <path d="M3 6h18M3 12h18M3 18h18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                                </svg>
                            </button>
                        )}

                        <div style={{ flex: 1 }} />

                        {/* Theme Toggle */}
                        <button
                            onClick={toggleTheme}
                            className="press"
                            style={{
                                width: 36, height: 36, borderRadius: 8, border: "none",
                                background: "transparent", display: "flex", alignItems: "center",
                                justifyContent: "center", cursor: "pointer", color: "var(--t3)",
                                marginRight: 4,
                            }}
                            aria-label={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
                            title={theme === "dark" ? "Light mode" : "Dark mode"}
                        >
                            {theme === "dark" ? (
                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                                    <circle cx="12" cy="12" r="5" stroke="currentColor" strokeWidth="2" />
                                    <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                                </svg>
                            ) : (
                                <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                                    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                                </svg>
                            )}
                        </button>

                        {/* Avatar */}
                        <div
                            className="press"
                            style={{
                                width: 32, height: 32, borderRadius: "50%",
                                background: "linear-gradient(135deg, #111112 0%, #3f3f46 100%)",
                                display: "flex", alignItems: "center",
                                justifyContent: "center", color: "var(--white)", fontSize: 13,
                                fontWeight: 700, cursor: "pointer", marginRight: 4,
                                boxShadow: "0 2px 8px rgba(0,0,0,0.15), inset 0 1px 0 rgba(255,255,255,0.2)",
                            }}
                            title="Dan Voulez"
                        >
                            D
                        </div>
                    </div>
                </header>

                {/* Page content */}
                <div className="page-enter" style={{ minHeight: "calc(100vh - 48px)" }}>
                    {children}
                </div>
            </div>
        </div>
    );
}
