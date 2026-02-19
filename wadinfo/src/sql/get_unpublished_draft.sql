-- Get the most recent unpublished draft for a user (for resuming)
SELECT draft_id, uploader_id, upload_id, title, author, description, ai_enabled, created_at, updated_at, status, file_sha256, file_size
FROM wad_drafts
WHERE uploader_id = $1 AND status = 'draft'
ORDER BY updated_at DESC
LIMIT 1;
