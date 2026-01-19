SELECT *
FROM wad_map_analysis
WHERE wad_id = $1::uuid
AND map_name = $2;