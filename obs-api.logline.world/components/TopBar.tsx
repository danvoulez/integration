"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useState, useEffect } from "react";

interface TopBarProps {
  title?: string;
  showBack?: boolean;
  backHref?: string;
  showSearch?: boolean;
  onSearch?: (q: string) => void;
}

export default function TopBar({ title = "UBL Workspace", showBack, backHref, showSearch, onSearch }: TopBarProps) {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [searchVal, setSearchVal] = useState("");
  const [scrolled, setScrolled] = useState(false);
  const pathname = usePathname();
  const router = useRouter();

  const isEco = pathname === "/" || pathname === "";
  const isApps = pathname.startsWith("/apps");

  useEffect(() => {
    const fn = () => setScrolled(window.scrollY > 2);
    window.addEventListener("scroll", fn, { passive: true });
    return () => window.removeEventListener("scroll", fn);
  }, []);

  useEffect(() => {
    document.body.style.overflow = drawerOpen ? "hidden" : "";
    return () => { document.body.style.overflow = ""; };
  }, [drawerOpen]);

  return (
    <>
      {/* ── Backdrop ── */}
      <div
        onClick={() => setDrawerOpen(false)}
        style={{
          position: "fixed", inset: 0, zIndex: 40,
          background: "rgba(10,10,10,0.5)",
          backdropFilter: "blur(3px)",
          WebkitBackdropFilter: "blur(3px)",
          opacity: drawerOpen ? 1 : 0,
          pointerEvents: drawerOpen ? "auto" : "none",
          transition: "opacity 0.22s ease",
        }}
      />

      {/* ── Drawer ── */}
      <nav
        style={{
          position: "fixed", top: 0, left: 0, height: "100%", zIndex: 50,
          width: 280, background: "var(--black)", color: "var(--white)",
          transform: drawerOpen ? "translateX(0)" : "translateX(-100%)",
          transition: "transform 0.4s cubic-bezier(0.16, 1, 0.3, 1)",
          display: "flex", flexDirection: "column",
          boxShadow: drawerOpen ? "4px 0 24px rgba(0,0,0,0.5)" : "none",
        }}
      >
        {/* Header */}
        <div style={{ padding: "36px 24px 28px", borderBottom: "1px solid rgba(255,255,255,0.08)" }}>
          <p style={{ margin: 0, fontSize: 10, fontWeight: 700, letterSpacing: "0.16em", textTransform: "uppercase", color: "#555", marginBottom: 10 }}>
            UBL System
          </p>
          <p style={{ margin: 0, fontSize: 20, fontWeight: 800, letterSpacing: "-0.02em", color: "#fff", lineHeight: 1 }}>
            Workspace
          </p>
        </div>

        {/* Nav items */}
        <div style={{ padding: "16px 12px", flex: 1 }}>
          {[
            {
              href: "/", label: "Ecossistema", active: isEco,
              icon: <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="1.8" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" strokeWidth="1.8" /><path d="M2 12h20" stroke="currentColor" strokeWidth="1.8" /></svg>,
            },
            {
              href: "/apps", label: "Apps", active: isApps,
              icon: <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><rect x="2" y="3" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.8" /><rect x="14" y="3" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.8" /><rect x="2" y="13" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.8" /><rect x="14" y="13" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.8" /></svg>,
            },
          ].map(({ href, label, active, icon }) => (
            <Link
              key={href} href={href}
              onClick={() => setDrawerOpen(false)}
              style={{
                display: "flex", alignItems: "center", gap: 12,
                padding: "10px 12px", borderRadius: 4,
                background: active ? "#fff" : "transparent",
                color: active ? "#0a0a0a" : "#888",
                fontWeight: active ? 700 : 500,
                fontSize: 13, textDecoration: "none",
                letterSpacing: "0.01em",
                marginBottom: 2,
                transition: "background 0.14s, color 0.14s",
              }}
            >
              {icon}
              {label}
              {active && <div style={{ marginLeft: "auto", width: 5, height: 5, borderRadius: "50%", background: "#16a34a" }} />}
            </Link>
          ))}
        </div>

        <div style={{ padding: "16px 20px 36px", borderTop: "1px solid #1a1a1a" }}>
          <p style={{ margin: 0, fontSize: 10, color: "#444", letterSpacing: "0.08em" }}>v0.1.0 — UBL Workspace</p>
        </div>
      </nav>

      {/* ── AppBar ── */}
      <header className={scrolled ? "glass-header" : ""} style={{
        position: "sticky", top: 0, zIndex: 30,
        background: scrolled ? "transparent" : "transparent", /* handled by class when scrolled */
        borderBottom: scrolled ? "1px solid var(--line)" : "1px solid transparent",
        transition: "border-color 0.3s ease, background 0.3s ease, backdrop-filter 0.3s ease",
      }}>
        <div style={{ display: "flex", alignItems: "center", height: 52, padding: "0 4px 0 8px", gap: 0 }}>
          {/* Left button */}
          {showBack ? (
            <button
              onClick={() => backHref ? router.push(backHref) : router.back()}
              className="press"
              style={{
                width: 40, height: 40, borderRadius: 4, border: "none",
                background: "transparent", display: "flex", alignItems: "center",
                justifyContent: "center", cursor: "pointer", color: "#0a0a0a", flexShrink: 0,
              }}
              aria-label="Voltar"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                <path d="M19 12H5M5 12l7-7M5 12l7 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          ) : (
            <button
              onClick={() => setDrawerOpen(true)}
              className="press"
              style={{
                width: 40, height: 40, borderRadius: 4, border: "none",
                background: "transparent", display: "flex", alignItems: "center",
                justifyContent: "center", cursor: "pointer", color: "#0a0a0a", flexShrink: 0,
              }}
              aria-label="Menu"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                <path d="M3 6h18M3 12h18M3 18h18" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
          )}

          {/* Title */}
          <div style={{ flex: 1, padding: "0 8px" }}>
            <p style={{
              margin: 0, fontSize: 14, fontWeight: 800,
              letterSpacing: "-0.01em", color: "#0a0a0a",
              whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
            }}>
              {title}
            </p>
          </div>

          {/* Right icons */}
          <button
            className="press"
            style={{ width: 40, height: 40, borderRadius: 4, border: "none", background: "transparent", display: "flex", alignItems: "center", justifyContent: "center", cursor: "pointer", color: "#999" }}
            aria-label="Sync"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
              <path d="M4 4v6h6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              <path d="M20 20v-6h-6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              <path d="M20.49 9A9 9 0 0 0 5.64 5.64L4 10m16 4-1.64 4.36A9 9 0 0 1 3.51 15" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>

          <div
            className="press"
            style={{
              width: 32, height: 32, borderRadius: "50%",
              background: "linear-gradient(135deg, #111112 0%, #3f3f46 100%)",
              display: "flex", alignItems: "center",
              justifyContent: "center", color: "var(--white)", fontSize: 13,
              fontWeight: 700, cursor: "pointer", marginRight: 8,
              boxShadow: "0 2px 8px rgba(0,0,0,0.15), inset 0 1px 0 rgba(255,255,255,0.2)",
              letterSpacing: "-0.01em",
            }}
            title="Dan Voulez"
          >
            D
          </div>
        </div>

        {showSearch && (
          <div style={{ padding: "0 12px 12px" }}>
            <div
              style={{
                display: "flex", alignItems: "center", gap: 8,
                padding: "0 14px", height: 40,
                border: "1px solid var(--line)",
                borderRadius: 8, background: "var(--bg-card)",
                boxShadow: "inset 0 2px 4px rgba(0,0,0,0.02)",
                transition: "all 0.2s ease",
              }}
              onFocus={(e) => { e.currentTarget.style.borderColor = "var(--black)"; e.currentTarget.style.boxShadow = "0 0 0 1px var(--black), inset 0 2px 4px rgba(0,0,0,0.02)"; }}
              onBlur={(e) => { e.currentTarget.style.borderColor = "var(--line)"; e.currentTarget.style.boxShadow = "inset 0 2px 4px rgba(0,0,0,0.02)"; }}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" style={{ flexShrink: 0, color: "var(--t3)" }}>
                <circle cx="11" cy="11" r="8" stroke="currentColor" strokeWidth="2" />
                <path d="m21 21-4.35-4.35" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
              <input
                type="text"
                placeholder="Buscar..."
                value={searchVal}
                onChange={(e) => { setSearchVal(e.target.value); onSearch?.(e.target.value); }}
                style={{
                  flex: 1, background: "transparent", border: "none", outline: "none",
                  fontSize: 13, color: "#0a0a0a", fontFamily: "inherit",
                }}
              />
              {searchVal && (
                <button
                  onClick={() => { setSearchVal(""); onSearch?.(""); }}
                  className="press"
                  style={{ border: "none", background: "transparent", cursor: "pointer", padding: 2, display: "flex", color: "#999" }}
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
                    <path d="M18 6 6 18M6 6l12 12" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
                  </svg>
                </button>
              )}
            </div>
          </div>
        )}
      </header>
    </>
  );
}
