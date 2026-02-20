-- Delete a WAD (only if owned by user).
-- Returns the wad_id, file_sha256, and original filename if deleted.
DELETE FROM wads
WHERE wad_id = $1
  AND uploader_id = $2
RETURNING wad_id, sha256 as file_sha256, preferred_filename as filename;
