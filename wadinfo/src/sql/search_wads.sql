select
  wad_id,
  sha1,
  filename,
  wad_type,
  byte_size,
  greatest(
    similarity(filename, $1),
    case
      when filename ilike '%' || $1 || '%' then 0.9
      else 0
    end
  ) as rank
from wads
where filename % $1
   or filename ilike '%' || $1 || '%'
order by rank desc, length(filename), filename
offset $2
limit $3;
