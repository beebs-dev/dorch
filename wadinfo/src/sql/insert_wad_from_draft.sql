-- Insert a minimal WAD row from a published draft
-- Returns the wad_id (either new or existing if sha1 matches)
WITH input AS (
  SELECT
    gen_random_uuid() AS wad_id,
    $1::char(40) AS sha1,
    $2::char(64) AS sha256,
    $3::text AS title,
    $4::text AS preferred_filename,
    $5::bigint AS file_size_bytes,
    $6::text AS file_url
)
INSERT INTO wads (
  wad_id,
  sha1,
  sha256,
  title,
  preferred_filename,
  file_type,
  file_size_bytes,
  file_url,
  meta_json
)
SELECT
  wad_id,
  sha1,
  sha256,
  title,
  preferred_filename,
  'PWAD',
  file_size_bytes,
  file_url,
  jsonb_build_object('id', wad_id, 'title', title)
FROM input
ON CONFLICT (sha1) DO UPDATE SET
  sha256 = COALESCE(excluded.sha256, wads.sha256),
  title = COALESCE(excluded.title, wads.title),
  preferred_filename = COALESCE(excluded.preferred_filename, wads.preferred_filename),
  file_size_bytes = COALESCE(excluded.file_size_bytes, wads.file_size_bytes),
  file_url = COALESCE(excluded.file_url, wads.file_url),
  updated_at = now()
RETURNING wads.wad_id;
