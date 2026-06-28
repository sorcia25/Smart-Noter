-- Cloud provider API keys (Sub-6). Ciphertext is DPAPI-encrypted; plaintext is
-- never stored. One row per provider id.
CREATE TABLE provider_secrets (
    provider   TEXT PRIMARY KEY,       -- 'openai' | 'anthropic' | 'azure'
    ciphertext BLOB NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
