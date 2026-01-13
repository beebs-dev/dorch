select
  wad_id,
  meta_json
from wads
where wad_id = $1::uuid;
