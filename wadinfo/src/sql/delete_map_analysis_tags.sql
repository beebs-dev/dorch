DELETE FROM wad_map_analysis_tags
WHERE wad_id = $1::uuid AND map_name = $2::text;