-- Delete a draft (only if owned by user)
DELETE FROM wad_drafts
WHERE draft_id = $1 AND uploader_id = $2
RETURNING draft_id, upload_id;
