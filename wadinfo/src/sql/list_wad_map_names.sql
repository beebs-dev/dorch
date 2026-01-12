SELECT map_name
FROM wad_maps
WHERE wad_id = $1
ORDER BY map_name ASC;