import postgres from 'postgres';
import { drizzle, type PostgresJsDatabase } from 'drizzle-orm/postgres-js';
import type { Sql } from 'postgres';
import * as schema from './schema';

let _client: Sql | null = null;
let _db: PostgresJsDatabase<typeof schema> | null = null;

function getConnectionString(): string {
  const cs =
    process.env.SUPABASE_DB_URL ??
    process.env.DATABASE_URL ??
    process.env.DATABASE_URL_UNPOOLED ??
    process.env.POSTGRES_URL;
  if (!cs) {
    throw new Error('SUPABASE_DB_URL (or DATABASE_URL) is not set.');
  }
  return cs;
}

function getClient(): Sql {
  if (!_client) {
    _client = postgres(getConnectionString(), { prepare: false, max: 1 });
  }
  return _client;
}

export function getDb(): PostgresJsDatabase<typeof schema> {
  if (!_db) {
    _db = drizzle(getClient(), { schema });
  }
  return _db;
}

export const db = new Proxy({} as PostgresJsDatabase<typeof schema>, {
  get(_target, prop, receiver) {
    return Reflect.get(getDb(), prop, receiver);
  },
});

const sqlCallable = ((...args: unknown[]) => {
  const client = getClient() as unknown as (...innerArgs: unknown[]) => unknown;
  return client(...args);
}) as unknown as Sql;

export const sql = new Proxy(sqlCallable, {
  get(_target, prop, receiver) {
    return Reflect.get(getClient(), prop, receiver);
  },
});
