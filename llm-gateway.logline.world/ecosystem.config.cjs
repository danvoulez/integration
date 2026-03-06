module.exports = {
    apps: [
        {
            name: 'llm-gateway',
            script: 'cargo',
            args: 'run --release',
            cwd: '/Users/ubl-ops/llm-gateway',
            autorestart: true,
            max_restarts: 10,
            min_uptime: '5s',
            restart_delay: 5000,
            env: {
                RUST_LOG: 'info',
            }
        },
        {
            name: 'oz-local',
            script: 'node',
            args: 'tools/oz-local.mjs',
            cwd: '/Users/ubl-ops/vvtv-platform',
            autorestart: true,
            max_restarts: 20,
            min_uptime: '10s',
            restart_delay: 15000,
            env: {
                OZ_RUN_ONCE: 'false',
                OZ_DRY_RUN: 'false',
                OZ_INTERVAL: '60',
            }
        },
        {
            name: 'vvtv-dashboard',
            script: 'node',
            args: 'tools/serve-observability-live.mjs',
            cwd: '/Users/ubl-ops/vvtv-platform',
            autorestart: true,
            max_restarts: 10,
            min_uptime: '5s',
            restart_delay: 5000,
        }
    ]
};
