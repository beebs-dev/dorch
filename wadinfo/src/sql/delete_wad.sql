-- Delete a WAD (only if owned by user via draft)
-- Returns the wad_id, file_sha256, and original filename if deleted.
DELETE FROM wads w
USING wad_drafts d
WHERE w.wad_id = $1
  AND d.wad_id = w.wad_id
  AND d.uploader_id = $2
RETURNING w.wad_id, w.sha256 as file_sha256, d.filename;
