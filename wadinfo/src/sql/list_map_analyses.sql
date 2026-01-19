SELECT *
FROM wad_map_analysis
WHERE wad_id = $1::uuid
ORDER BY map_name ASC;