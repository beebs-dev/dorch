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

  monster_total,
  uv_monsters,
  hmp_monsters,
  htr_monsters,

  zombieman_count,
  shotgun_guy_count,
  chaingun_guy_count,
  imp_count,
  demon_count,
  spectre_count,
  cacodemon_count,
  lost_soul_count,
  pain_elemental_count,
  revenant_count,
  mancubus_count,
  arachnotron_count,
  hell_knight_count,
  baron_count,
  archvile_count,
  cyberdemon_count,
  spider_mastermind_count,

  keys,
  doc
) values (
  $1,  -- wad_id
  $2,  -- map_name
  $3,  -- format
  $4,  -- compatibility

  $5,  -- things
  $6,  -- linedefs
  $7,  -- sidedefs
  $8,  -- vertices
  $9,  -- sectors
  $10, -- segs
  $11, -- ssectors
  $12, -- nodes

  $13, -- teleports
  $14, -- secret_exit

  $15, -- monster_total
  $16, -- uv_monsters
  $17, -- hmp_monsters
  $18, -- htr_monsters

  $19, -- zombieman_count
  $20, -- shotgun_guy_count
  $21, -- chaingun_guy_count
  $22, -- imp_count
  $23, -- demon_count
  $24, -- spectre_count
  $25, -- cacodemon_count
  $26, -- lost_soul_count
  $27, -- pain_elemental_count
  $28, -- revenant_count
  $29, -- mancubus_count
  $30, -- arachnotron_count
  $31, -- hell_knight_count
  $32, -- baron_count
  $33, -- archvile_count
  $34, -- cyberdemon_count
  $35, -- spider_mastermind_count

  $36, -- keys (text[])
  $37  -- doc (jsonb)
)
on conflict (wad_id, map_name) do update set
  format        = excluded.format,
  compatibility = excluded.compatibility,

  things        = excluded.things,
  linedefs      = excluded.linedefs,
  sidedefs      = excluded.sidedefs,
  vertices      = excluded.vertices,
  sectors       = excluded.sectors,
  segs          = excluded.segs,
  ssectors      = excluded.ssectors,
  nodes         = excluded.nodes,

  teleports     = excluded.teleports,
  secret_exit   = excluded.secret_exit,

  monster_total = excluded.monster_total,
  uv_monsters   = excluded.uv_monsters,
  hmp_monsters  = excluded.hmp_monsters,
  htr_monsters  = excluded.htr_monsters,

  zombieman_count         = excluded.zombieman_count,
  shotgun_guy_count       = excluded.shotgun_guy_count,
  chaingun_guy_count      = excluded.chaingun_guy_count,
  imp_count               = excluded.imp_count,
  demon_count             = excluded.demon_count,
  spectre_count           = excluded.spectre_count,
  cacodemon_count         = excluded.cacodemon_count,
  lost_soul_count         = excluded.lost_soul_count,
  pain_elemental_count    = excluded.pain_elemental_count,
  revenant_count          = excluded.revenant_count,
  mancubus_count          = excluded.mancubus_count,
  arachnotron_count       = excluded.arachnotron_count,
  hell_knight_count       = excluded.hell_knight_count,
  baron_count             = excluded.baron_count,
  archvile_count          = excluded.archvile_count,
  cyberdemon_count        = excluded.cyberdemon_count,
  spider_mastermind_count = excluded.spider_mastermind_count,

  keys         = excluded.keys,
  doc          = excluded.doc;
