-- Public read: only return rows that would appear in search/list.
select
  wad_id,
  meta_json
from wads
where wad_id = $1::uuid
  and hidden = false
  and can_download = true;
