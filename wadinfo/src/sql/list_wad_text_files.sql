select source, name, contents
from wad_text_files
where wad_id = $1::uuid
order by ord asc;
