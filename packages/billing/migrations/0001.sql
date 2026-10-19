CREATE TABLE billing_delivery (event_id text PRIMARY KEY);
CREATE TABLE entitlements (
 subscription_id text PRIMARY KEY, user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
 product_id text NOT NULL, version bigint NOT NULL, active boolean NOT NULL, valid_until bigint NOT NULL
);
CREATE INDEX entitlements_user_idx ON entitlements(user_id);
