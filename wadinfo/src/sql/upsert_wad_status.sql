-- Upsert WAD status
INSERT INTO wad_status (wad_id, status)
VALUES ($1, $2)
ON CONFLICT (wad_id) DO UPDATE SET status = excluded.status;
