select
  w.wad_id,
  w.meta_json
from wads w
where w.wad_id = any($1::uuid[]);
