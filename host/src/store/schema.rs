//! The database schema. Each entry upgrades it by one version (`PRAGMA user_version`).
//! Never edit a shipped entry; append a new one.

pub const MIGRATIONS: &[&str] = &[
    // 1: threads, the event log (the source of truth), and the tables kept in step with it.
    "CREATE TABLE threads (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        cli TEXT NOT NULL,
        config_dir TEXT NOT NULL,
        cwd TEXT NOT NULL,
        model TEXT,
        effort TEXT,
        permission_mode TEXT NOT NULL,
        native_session_id TEXT,
        items_used INTEGER NOT NULL DEFAULT 0,
        status TEXT NOT NULL DEFAULT 'idle',
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE events (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
        turn_id TEXT,
        at_ms INTEGER NOT NULL,
        body TEXT NOT NULL
    );
    CREATE INDEX events_by_thread ON events(thread_id, seq);
    CREATE TABLE turns (
        id TEXT PRIMARY KEY,
        thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
        prompt TEXT,
        status TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        ended_at INTEGER,
        cost_usd REAL,
        error TEXT,
        checkpoint_before TEXT,
        checkpoint_after TEXT
    );
    CREATE INDEX turns_by_thread ON turns(thread_id, started_at);
    CREATE INDEX turns_running ON turns(status) WHERE status = 'running';
    CREATE TABLE requests (
        thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
        request_id TEXT NOT NULL,
        turn_id TEXT,
        kind TEXT NOT NULL,
        tool_name TEXT NOT NULL,
        input TEXT NOT NULL,
        status TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        resolved_at INTEGER,
        PRIMARY KEY (thread_id, request_id)
    );
    CREATE INDEX requests_pending ON requests(status) WHERE status = 'pending';
    CREATE TABLE subagents (
        thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
        task_id TEXT NOT NULL,
        tool_use_id TEXT,
        description TEXT NOT NULL,
        status TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        ended_at INTEGER,
        PRIMARY KEY (thread_id, task_id)
    );",
    // 2: the control room. Teams; a thread's team, parent, run (root) and branch; results
    // handed back; queued messages; merges (so they can be undone).
    "CREATE TABLE teams (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        cli TEXT NOT NULL,
        config_dir TEXT NOT NULL,
        model TEXT,
        effort TEXT,
        created_at INTEGER NOT NULL
    );
    ALTER TABLE threads ADD COLUMN team_id TEXT REFERENCES teams(id);
    ALTER TABLE threads ADD COLUMN parent_id TEXT REFERENCES threads(id);
    ALTER TABLE threads ADD COLUMN root_id TEXT;
    ALTER TABLE threads ADD COLUMN depth INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE threads ADD COLUMN branch TEXT;
    ALTER TABLE threads ADD COLUMN client_request_id TEXT;
    CREATE UNIQUE INDEX threads_by_request ON threads(parent_id, client_request_id) WHERE client_request_id IS NOT NULL;
    CREATE INDEX threads_by_parent ON threads(parent_id);
    CREATE TABLE results (
        thread_id TEXT PRIMARY KEY REFERENCES threads(id) ON DELETE CASCADE,
        parent_id TEXT NOT NULL,
        state TEXT NOT NULL,
        summary TEXT,
        created_at INTEGER NOT NULL,
        delivered_at INTEGER
    );
    CREATE TABLE mailbox (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
        from_thread TEXT,
        text TEXT NOT NULL,
        state TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE INDEX mailbox_queued ON mailbox(thread_id, state);
    CREATE TABLE merges (
        id TEXT PRIMARY KEY,
        thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
        from_thread TEXT NOT NULL,
        cwd TEXT NOT NULL,
        branch TEXT NOT NULL,
        pre_head TEXT NOT NULL,
        merge_commit TEXT NOT NULL,
        checkpoint TEXT NOT NULL,
        state TEXT NOT NULL,
        created_at INTEGER NOT NULL
    );",
    // 3: quota and the inbox. Each team's usage windows and how long it is out of quota;
    // host settings; when a thread continues after a limit resets; how many automatic
    // continues in a row a thread has had (capped, so a turn that keeps crashing stops).
    "CREATE TABLE team_usage (
        team_id TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
        window TEXT NOT NULL,
        utilization REAL,
        resets_at INTEGER,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (team_id, window)
    );
    ALTER TABLE teams ADD COLUMN limited_until INTEGER;
    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    ALTER TABLE threads ADD COLUMN resume_at INTEGER;
    ALTER TABLE threads ADD COLUMN auto_continues INTEGER NOT NULL DEFAULT 0;",
    // 4: paired devices (each holds its own key; revoking deletes it), and which device
    // answered each request ("local" for a client on the host itself).
    "CREATE TABLE devices (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        role TEXT NOT NULL,
        public_key TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        last_seen_at INTEGER
    );
    ALTER TABLE requests ADD COLUMN answered_by TEXT;",
];
