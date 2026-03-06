# Secrets Runbook (Doppler)

Status: active  
Updated: 2026-03-05

## 1) Decision

- **Vault central**: Doppler (`project=logline-ecosystem`)
- **Config padrão**: `dev`
- **Regra**: segredo vive no Doppler; serviços recebem segredo em runtime via `doppler run`.

## 2) Bootstrap

```bash
cd /Users/ubl-ops/Integration
./scripts/doppler-setup.sh --project logline-ecosystem --config dev
```

Esse comando:
- valida Doppler CLI/login
- garante o projeto no Doppler
- escreve `doppler.yaml` no root e nos serviços

## 3) Manifesto de chaves

Arquivo canônico:
- `/Users/ubl-ops/Integration/secrets/doppler-secrets-manifest.tsv`

Notas de alinhamento de runtime (Code247):
- `LINEAR_WEBHOOK_SECRET` (alias aceito: `LINEAR_WEBHOOK_SIGNING_SECRET`)
- `SUPABASE_SERVICE_ROLE_KEY` (alias aceito: `SUPABASE_SERVICE_KEY`)
- `CODE247_SUPABASE_TENANT_ID` e `CODE247_SUPABASE_APP_ID` são necessários para sync Supabase do Code247
- `GITHUB_TOKEN` é obrigatório no runtime atual do Code247 (GitHub App fica como trilha futura)

Campos:
- `key`
- `required` (`required|optional`)
- `services`
- `notes`

## 4) Auditoria (o que falta preencher)

Apenas obrigatórias:

```bash
cd /Users/ubl-ops/Integration
./scripts/doppler-secrets-audit.sh --project logline-ecosystem --config dev
```

Obrigatórias + opcionais:

```bash
./scripts/doppler-secrets-audit.sh --project logline-ecosystem --config dev --all
```

## 5) Como preencher segredo no Doppler

```bash
doppler secrets set SUPABASE_SERVICE_ROLE_KEY=... --project logline-ecosystem --config dev
```

## 6) Runtime PM2

`ecosystem.config.cjs` já está preparado para executar serviços com `doppler run` por app.

Subir stack:

```bash
pm2 start /Users/ubl-ops/Integration/ecosystem.config.cjs
```

Recarregar env após mudança de segredo:

```bash
pm2 restart all --update-env
```

## 7) Rotação

1. atualizar valor no Doppler
2. reiniciar somente serviços afetados (`pm2 restart <service> --update-env`)
3. validar health endpoint
4. rodar auditoria novamente

## 8) Segurança operacional

- não commitar `.env` com segredo
- não imprimir segredo em log
- usar token por serviço (least privilege)
- manter `obs-api`, `code247`, `edge-control`, `llm-gateway`, `logic` no mesmo padrão de injeção via Doppler
