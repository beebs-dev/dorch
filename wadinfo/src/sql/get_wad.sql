select
  wad_id,
  meta_json,
  uploader_id,
  description
from wads
where wad_id = $1::uuid;
