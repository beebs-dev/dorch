-- Mark a draft as published (only if owned by user and has upload_id)
UPDATE wad_drafts
SET status = 'published', updated_at = $3
WHERE draft_id = $1 AND uploader_id = $2 AND upload_id IS NOT NULL AND status = 'draft'
RETURNING draft_id, uploader_id, upload_id, title, author, description, ai_enabled, created_at, updated_at, status, file_sha256, file_size;
