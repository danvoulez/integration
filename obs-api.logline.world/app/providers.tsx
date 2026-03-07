'use client';

import React, { createContext, useContext, useEffect, useState } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { Session, User } from '@supabase/supabase-js';
import { getSupabaseBrowserClient } from '@/lib/auth/supabase-browser';

type AuthState = {
  session: Session | null;
  user: User | null;
  loading: boolean;
  isRecovery: boolean;
};

const AuthContext = createContext<AuthState>({
  session: null,
  user: null,
  loading: true,
  isRecovery: false,
});

export function useAuth() {
  return useContext(AuthContext);
}

function AuthProvider({ children }: { children: React.ReactNode }) {
  const [auth, setAuth] = useState<AuthState>({
    session: null,
    user: null,
    loading: true,
    isRecovery: false,
  });

  useEffect(() => {
    const supabase = getSupabaseBrowserClient();

    supabase.auth.getSession().then(({ data: { session: s } }) => {
      setAuth((prev) => ({ ...prev, session: s, user: s?.user ?? null, loading: false }));
    });

    const { data: { subscription } } = supabase.auth.onAuthStateChange(
      (event, session) => {
        setAuth((prev) => ({
          session,
          user: session?.user ?? null,
          loading: false,
          isRecovery: event === 'PASSWORD_RECOVERY' ? true : event === 'USER_UPDATED' || event === 'SIGNED_IN' || event === 'SIGNED_OUT' ? false : prev.isRecovery,
        }));
      },
    );

    return () => subscription.unsubscribe();
  }, []);

  return (
    <AuthContext.Provider value={auth}>
      {children}
    </AuthContext.Provider>
  );
}

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime:            30_000,
            refetchOnWindowFocus: false,
            retry:                1,
          },
        },
      })
  );

  return (
    <AuthProvider>
      <QueryClientProvider client={queryClient}>
        {children}
      </QueryClientProvider>
    </AuthProvider>
  );
}
