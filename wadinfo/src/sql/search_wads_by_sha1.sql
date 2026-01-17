-- Fast path: exact SHA1 match.
select
  wad_id,
  meta_json
from wads
where hidden = false
  and can_download = true
  and sha1 = $1::char(40)
limit 1;
