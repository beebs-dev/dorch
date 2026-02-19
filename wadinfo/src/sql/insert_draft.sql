-- Insert a new draft for a user
INSERT INTO wad_drafts (draft_id, uploader_id, created_at, updated_at, status)
VALUES ($1, $2, $3, $3, 'draft')
RETURNING draft_id, uploader_id, upload_id, title, author, description, ai_enabled, created_at, updated_at, status, file_sha256, file_size;
