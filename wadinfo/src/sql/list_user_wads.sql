-- List all WADs uploaded/published by a user
SELECT 
    wad_id, 
    sha1, 
    title, 
    preferred_filename, 
    file_size_bytes, 
    file_url, 
    EXTRACT(EPOCH FROM created_at)::bigint * 1000 AS created_at,
    EXTRACT(EPOCH FROM updated_at)::bigint * 1000 AS updated_at
FROM wads
WHERE uploader_id = $1
ORDER BY updated_at DESC;
