-- KEYS[1] = hash key
-- ARGV = [field1, value1, field2, value2, ..., game_id, channel, ttl]

local key = KEYS[1]
local argc = #ARGV
if argc < 3 then
  return redis.error_reply("missing game_id, channel, or ttl arg")
end

local game_id = ARGV[argc - 2]
local channel = ARGV[argc - 1]
local ttl     = ARGV[argc]
local pair_count = argc - 3

if (pair_count % 2) ~= 0 then
  return redis.error_reply("field/value args must be even; game_id, channel, and ttl must be last")
end

-- Build list of fields for HMGET
local fields = {}
local fcount = pair_count / 2
local idx = 1
for i = 1, pair_count, 2 do
  fields[idx] = ARGV[i]
  idx = idx + 1
end

-- Fetch old values in one call
local oldvals = {}
if fcount > 0 then
  oldvals = redis.call("HMGET", key, unpack(fields))
end

-- Determine changes
local changed_members = {} -- flat list for HSET
local changed_obj = {
  game_id = game_id
}
local changed_pairs = 0

idx = 1
for i = 1, pair_count, 2 do
  local field = ARGV[i]
  local newv  = ARGV[i + 1]
  local oldv  = oldvals[idx]
  idx = idx + 1

  -- Compare raw strings; missing field counts as change
  if oldv ~= newv then
    table.insert(changed_members, field)
    table.insert(changed_members, newv)

    changed_obj[field] = tonumber(newv) or newv
    changed_pairs = changed_pairs + 1
  end
end

-- Apply only changed fields
if changed_pairs > 0 then
  redis.call("HSET", key, unpack(changed_members))
end

-- Update TTL (independent of whether fields changed)
redis.call("EXPIRE", key, ttl)

-- Publish only if something changed
if changed_pairs > 0 then
  local json = cjson.encode(changed_obj)
  redis.call("PUBLISH", channel, json)
end
