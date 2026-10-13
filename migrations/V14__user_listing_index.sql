-- The panel's default page order must not sort the entire account table.
-- The primary key breaks creation-time ties for stable pagination.
CREATE INDEX users_created_id_idx ON auth.users (created_at DESC, id DESC);
