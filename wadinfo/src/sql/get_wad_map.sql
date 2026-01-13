select
  w.wad_id,
  w.meta_json,
  wm.map_json
from wad_maps wm
join wads w on w.wad_id = wm.wad_id
where wm.wad_id = $1::uuid
  and wm.map_name = $2::text
limit 1;
