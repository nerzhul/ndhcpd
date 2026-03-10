-- Create api_tokens table for API authentication
CREATE TABLE IF NOT EXISTS api_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL,
    salt TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    last_used_at INTEGER,
    enabled INTEGER NOT NULL DEFAULT 1
);

-- Create index for fast token lookups
CREATE INDEX IF NOT EXISTS idx_api_tokens_enabled ON api_tokens(enabled);
