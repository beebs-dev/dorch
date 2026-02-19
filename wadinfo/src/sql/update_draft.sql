-- Update a draft (only if owned by user and still in draft status)
UPDATE wad_drafts
SET
  title = COALESCE($3, title),
  author = COALESCE($4, author),
  description = COALESCE($5, description),
  ai_enabled = COALESCE($6, ai_enabled),
  upload_id = COALESCE($7, upload_id),
  file_sha256 = COALESCE($8, file_sha256),
  file_size = COALESCE($9, file_size),
  updated_at = $10
WHERE draft_id = $1 AND uploader_id = $2 AND status = 'draft'
RETURNING draft_id, uploader_id, upload_id, title, author, description, ai_enabled, created_at, updated_at, status, file_sha256, file_size;
