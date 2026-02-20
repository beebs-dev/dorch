select
  wad_id,
  meta_json,
  uploader_id
from wads
where wad_id = $1::uuid;
