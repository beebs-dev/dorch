-- Update a WAD's editable fields (only by the uploader)
UPDATE wads
SET 
  title = COALESCE($3, title),
  meta_json = jsonb_set(
    meta_json,
    '{title}',
    COALESCE(to_jsonb($3::text), meta_json->'title')
  ),
  updated_at = now()
WHERE wad_id = $1 AND uploader_id = $2
RETURNING wad_id;
