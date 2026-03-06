import express from 'express';
import { loadConfig } from './config/index.js';
import { openDatabase, runMigrations } from './persistence/db.js';
import { JobsRepository } from './persistence/jobs.js';
import { CheckpointStore } from './control/checkpoint.js';
import { AuditLog } from './persistence/audit.js';
import { EvidenceStore } from './persistence/evidence.js';
import { Metrics } from './observability/metrics.js';
import { StateMachine } from './control/state-machine.js';
import { AnthropicAdapter } from './adapters/anthropic.js';
import { OllamaAdapter } from './adapters/ollama.js';
import { GitAdapter } from './adapters/git.js';
import { Pipeline } from './execution/pipeline.js';
import { WorkQueue } from './control/queue.js';
import { Scheduler } from './control/scheduler.js';
import { LinearAdapter } from './adapters/linear.js';
import { intakeStage } from './execution/stages/intake.js';
import { healthRouter } from './api/health.js';
import { dashboardRouter } from './api/dashboard.js';
import { createLogger } from './observability/logger.js';
import { FileWriterAdapter } from './adapters/file-writer.js';
import { SupabaseRealtimeAdapter } from './adapters/supabase-realtime.js';
import { WebhookAdapter } from './adapters/webhook.js';
import { EmailAdapter } from './adapters/email.js';
import { ExecutionLogger } from './persistence/execution-logger.js';
import { ConversationHandler } from './control/conversation-handler.js';

const bootstrap = async (): Promise<void> => {
const config = loadConfig();
const logger = createLogger(config.observability.logLevel);

const db = openDatabase(config.db.path);
runMigrations(db);

const jobs = new JobsRepository(db);
const checkpoints = new CheckpointStore(db);
const audit = new AuditLog(config.db.auditPath);
const evidence = new EvidenceStore(config.db.evidencePath);
const metrics = new Metrics();
const fsm = new StateMachine();

const anthropic = new AnthropicAdapter(config.anthropic.apiKey, config.anthropic.model);
const ollama = new OllamaAdapter(config.ollama.baseUrl, config.ollama.model);
const executionLogger = new ExecutionLogger(db);
const fileWriter = new FileWriterAdapter(config.repo.root);
const git = new GitAdapter(config.repo.root, config.github.remote, config.github.branch, config.github.token);
const linear = new LinearAdapter(config.linear.teamKey, config.linear.project);
const supabase = new SupabaseRealtimeAdapter(config.supabase.url, config.supabase.anonKey, config.supabase.channel);
const email = new EmailAdapter(config.email.apiUrl, config.email.apiKey, config.email.to);
const webhookAdapter = new WebhookAdapter(config.webhook.secret, executionLogger);
const conversationHandler = new ConversationHandler(anthropic, supabase, email, executionLogger, jobs);

await supabase.connect();
supabase.onMessage(async (msg) => {
  await conversationHandler.handleInbound({
    source: 'supabase',
    type: msg.type,
    content: msg.content,
    jobId: msg.jobId
  });
});

webhookAdapter.on('alert', async (payload) => {
  await conversationHandler.handleInbound({
    source: 'webhook',
    type: 'alert',
    content: payload.message,
    jobId: payload.jobId
  });
});

const pipeline = new Pipeline(
  jobs,
  checkpoints,
  audit,
  evidence,
  metrics,
  fsm,
  anthropic,
  ollama,
  git,
  fileWriter,
  executionLogger,
  conversationHandler,
  linear
);
const queue = new WorkQueue(jobs);

const scheduler = new Scheduler(queue, config.runtime.pollInterval, async () => intakeStage(linear));
scheduler.start();

setInterval(async () => {
  const job = queue.pull();
  if (!job) return;
  try {
    await pipeline.run(job);
    logger.info({ jobId: job.id }, 'job completed');
  } catch (error) {
    jobs.incrementRetries(job.id);
    jobs.updateStatus(job.id, 'FAILED', error instanceof Error ? error.message : 'unknown error');
    logger.error({ err: error, jobId: job.id }, 'job failed');
  }
}, 1000);

const healthApp = express();
healthApp.use('/', healthRouter(metrics));
healthApp.listen(config.observability.healthPort, () => {
  logger.info(`health/metrics listening on ${config.observability.healthPort}`);
});

const dashboardApp = express();
dashboardApp.use('/', dashboardRouter());
dashboardApp.listen(config.observability.dashboardPort, () => {
  logger.info(`dashboard listening on ${config.observability.dashboardPort}`);
});

const webhookApp = express();
webhookApp.use(express.json());
webhookApp.use('/', webhookAdapter.getRouter());
webhookApp.listen(config.webhook.port, () => {
  logger.info(`webhook server listening on ${config.webhook.port}`);
});

};

void bootstrap();
