insert into wad_maps (
  wad_id,
  map_name,
  format,
  compatibility,

  things,
  linedefs,
  sidedefs,
  vertices,
  sectors,
  segs,
  ssectors,
  nodes,

  teleports,
  secret_exit,

  keys,

  monster_total,
  uv_monsters,
  hmp_monsters,
  htr_monsters,

  item_total,
  uv_items,
  hmp_items,
  htr_items,

  title,
  music,
  metadata_source,

  map_json
) values (
  $1::uuid,
  $2::text,
  $3::text,
  $4::text,

  $5,  -- things
  $6,  -- linedefs
  $7,  -- sidedefs
  $8,  -- vertices
  $9,  -- sectors
  $10, -- segs
  $11, -- ssectors
  $12, -- nodes

  $13::boolean,
  $14::boolean,

  $15::text[],

  $16::bigint::int,
  $17::bigint::int,
  $18::bigint::int,
  $19::bigint::int,

  $20::bigint::int,
  $21::bigint::int,
  $22::bigint::int,
  $23::bigint::int,

  $24::text,
  $25::text,
  $26::text,

  $27::jsonb
)
;
