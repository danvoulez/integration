"use client";

import { useState } from "react";
import AppShell from "@/components/AppShell";
import { fuelByApps, fuelByTenants, fuelByUsers, totalFuel, FuelMetric } from "@/lib/data";
import { useAuth } from "./providers";
import { getSupabaseBrowserClient } from "@/lib/auth/supabase-browser";

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

// ── Reset Password Form (shown when user clicks reset link) ──
function ResetPasswordForm() {
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [done, setDone] = useState(false);

  const handleUpdate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (password !== confirm) { setError('Passwords do not match'); return; }
    if (password.length < 6) { setError('Password must be at least 6 characters'); return; }
    setError(null);
    setLoading(true);
    const supabase = getSupabaseBrowserClient();
    const { error: err } = await supabase.auth.updateUser({ password });
    setLoading(false);
    if (err) { setError(err.message); return; }
    setDone(true);
    setTimeout(() => window.location.reload(), 1500);
  };

  return (
    <div style={{ minHeight: '100dvh', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', background: 'var(--bg)', padding: 16 }}>
      <div style={{ width: '100%', maxWidth: 320 }}>
        <div style={{ textAlign: 'center', marginBottom: 24 }}>
          <h1 style={{ fontSize: 18, fontWeight: 800, color: 'var(--t1)' }}>UBL Workspace</h1>
          <p style={{ marginTop: 4, fontSize: 11, color: 'var(--t3)' }}>Set your new password</p>
        </div>
        {done ? (
          <p style={{ textAlign: 'center', fontSize: 12, color: 'var(--green)' }}>Password updated. Redirecting...</p>
        ) : (
          <form onSubmit={handleUpdate} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <input type="password" placeholder="New password" value={password} onChange={(e) => setPassword(e.target.value)} required minLength={6} style={{ width: '100%', background: 'var(--bg-card)', border: '1px solid var(--line)', borderRadius: 8, padding: '10px 12px', fontSize: 13, color: 'var(--t1)' }} />
            <input type="password" placeholder="Confirm password" value={confirm} onChange={(e) => setConfirm(e.target.value)} required minLength={6} style={{ width: '100%', background: 'var(--bg-card)', border: '1px solid var(--line)', borderRadius: 8, padding: '10px 12px', fontSize: 13, color: 'var(--t1)' }} />
            {error && <p style={{ fontSize: 10, color: 'var(--red)' }}>{error}</p>}
            <button type="submit" disabled={loading} style={{ width: '100%', padding: '10px 12px', borderRadius: 8, background: 'var(--blue)', border: 'none', fontSize: 13, fontWeight: 600, color: '#fff', cursor: 'pointer', opacity: loading ? 0.5 : 1 }}>
              {loading ? 'Updating...' : 'Update password'}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}

// ── Login Gate (sign in / sign up / forgot password) ──
function LoginGate() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<'login' | 'signup' | 'forgot'>('login');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setInfo(null);
    setLoading(true);

    const supabase = getSupabaseBrowserClient();

    if (mode === 'forgot') {
      const { error: resetErr } = await supabase.auth.resetPasswordForEmail(email, {
        redirectTo: `${window.location.origin}`,
      });
      setLoading(false);
      if (resetErr) { setError(resetErr.message); return; }
      setInfo('Check your email for a password reset link.');
      return;
    }

    const { error: authError } =
      mode === 'login'
        ? await supabase.auth.signInWithPassword({ email, password })
        : await supabase.auth.signUp({ email, password });

    setLoading(false);
    if (authError) {
      setError(authError.message);
    }
  };

  const inputStyle = { width: '100%', background: 'var(--bg-card)', border: '1px solid var(--line)', borderRadius: 8, padding: '10px 12px', fontSize: 13, color: 'var(--t1)' };

  return (
    <div style={{ minHeight: '100dvh', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', background: 'var(--bg)', padding: 16 }}>
      <div style={{ width: '100%', maxWidth: 320 }}>
        <div style={{ textAlign: 'center', marginBottom: 24 }}>
          <h1 style={{ fontSize: 18, fontWeight: 800, color: 'var(--t1)' }}>UBL Workspace</h1>
          <p style={{ marginTop: 4, fontSize: 11, color: 'var(--t3)' }}>LogLine Ops</p>
        </div>

        <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <input type="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} required style={inputStyle} />

          {mode !== 'forgot' && (
            <input type="password" placeholder="Password" value={password} onChange={(e) => setPassword(e.target.value)} required minLength={6} style={inputStyle} />
          )}

          {error && <p style={{ fontSize: 10, color: 'var(--red)' }}>{error}</p>}
          {info && <p style={{ fontSize: 10, color: 'var(--green)' }}>{info}</p>}

          <button type="submit" disabled={loading} style={{ width: '100%', padding: '10px 12px', borderRadius: 8, background: 'var(--blue)', border: 'none', fontSize: 13, fontWeight: 600, color: '#fff', cursor: 'pointer', opacity: loading ? 0.5 : 1 }}>
            {loading
              ? 'Please wait...'
              : mode === 'login'
                ? 'Sign in'
                : mode === 'signup'
                  ? 'Create account'
                  : 'Send reset link'}
          </button>
        </form>

        <div style={{ textAlign: 'center', marginTop: 16 }}>
          {mode === 'login' && (
            <>
              <p style={{ fontSize: 10, color: 'var(--t3)' }}>
                <button onClick={() => { setMode('forgot'); setError(null); setInfo(null); }} style={{ background: 'none', border: 'none', color: 'var(--t2)', textDecoration: 'underline', cursor: 'pointer', fontSize: 10 }}>
                  Forgot password?
                </button>
              </p>
              <p style={{ fontSize: 10, color: 'var(--t3)', marginTop: 6 }}>
                No account?{' '}
                <button onClick={() => { setMode('signup'); setError(null); setInfo(null); }} style={{ background: 'none', border: 'none', color: 'var(--t2)', textDecoration: 'underline', cursor: 'pointer', fontSize: 10 }}>
                  Sign up
                </button>
              </p>
            </>
          )}
          {mode === 'signup' && (
            <p style={{ fontSize: 10, color: 'var(--t3)' }}>
              Already have an account?{' '}
              <button onClick={() => { setMode('login'); setError(null); setInfo(null); }} style={{ background: 'none', border: 'none', color: 'var(--t2)', textDecoration: 'underline', cursor: 'pointer', fontSize: 10 }}>
                Sign in
              </button>
            </p>
          )}
          {mode === 'forgot' && (
            <p style={{ fontSize: 10, color: 'var(--t3)' }}>
              <button onClick={() => { setMode('login'); setError(null); setInfo(null); }} style={{ background: 'none', border: 'none', color: 'var(--t2)', textDecoration: 'underline', cursor: 'pointer', fontSize: 10 }}>
                Back to sign in
              </button>
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

// ── Main Page Export ──
export default function Page() {
  const { session, loading: authLoading, isRecovery } = useAuth();

  if (authLoading) {
    return (
      <div style={{ minHeight: '100dvh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--bg)' }}>
        <span style={{ fontSize: 11, fontWeight: 500, color: 'var(--t3)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>Initializing...</span>
      </div>
    );
  }

  if (isRecovery && session) {
    return <ResetPasswordForm />;
  }

  if (!session) {
    return <LoginGate />;
  }

  return <Dashboard />;
}

// ── Dashboard (previously EcossistemaPage) ──
function Dashboard() {
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
