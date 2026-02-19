-- List all drafts for a user, ordered by status (draft first) then updated_at DESC
SELECT draft_id, uploader_id, upload_id, title, author, description, ai_enabled, created_at, updated_at, status, file_sha256, file_size
FROM wad_drafts
WHERE uploader_id = $1
ORDER BY 
  CASE WHEN status = 'draft' THEN 0 ELSE 1 END,
  updated_at DESC;
