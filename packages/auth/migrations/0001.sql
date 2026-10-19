CREATE TABLE "user" (
 id text PRIMARY KEY, name text NOT NULL, email text NOT NULL UNIQUE,
 email_verified boolean NOT NULL, image text, created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL
);
CREATE TABLE session (
 id text PRIMARY KEY, user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
 token text NOT NULL UNIQUE, expires_at timestamptz NOT NULL, ip_address text, user_agent text,
 created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL
);
CREATE INDEX session_user_id_idx ON session(user_id);
CREATE TABLE account (
 id text PRIMARY KEY, user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
 account_id text NOT NULL, provider_id text NOT NULL, access_token text, refresh_token text, id_token text,
 access_token_expires_at timestamptz, refresh_token_expires_at timestamptz, scope text, password text,
 created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL
);
CREATE INDEX account_user_id_idx ON account(user_id);
CREATE TABLE verification (
 id text PRIMARY KEY, identifier text NOT NULL, value text NOT NULL, expires_at timestamptz NOT NULL,
 created_at timestamptz NOT NULL, updated_at timestamptz NOT NULL
);
CREATE INDEX verification_identifier_idx ON verification(identifier);
CREATE TABLE device_code (
 id text PRIMARY KEY, device_code varchar(191) NOT NULL UNIQUE, user_code varchar(191) NOT NULL UNIQUE,
 user_id text REFERENCES "user"(id) ON DELETE CASCADE, expires_at timestamptz NOT NULL, status text NOT NULL,
 last_polled_at timestamptz, polling_interval integer, client_id text, scope text
);
CREATE TABLE rate_limit (id text PRIMARY KEY, key text NOT NULL UNIQUE, count integer NOT NULL, last_request bigint NOT NULL);
