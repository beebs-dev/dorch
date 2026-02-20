-- Get a draft by ID
SELECT draft_id, uploader_id, upload_id, title, author, description, ai_enabled, created_at, updated_at, status, file_sha256, file_size, filename, file_sha1, wad_id
FROM wad_drafts
WHERE draft_id = $1;
