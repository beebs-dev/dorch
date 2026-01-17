-- KEYS[1] = hash key
-- ARGV = [set_pair_count, del_count, field1, value1, field2, value2, ..., del_field1, del_field2, ..., game_id, channel, ttl]

local key = KEYS[1]
local argc = #ARGV
if argc < 5 then
  return redis.error_reply("missing counts, game_id, channel, or ttl arg")
end

local set_pair_count = tonumber(ARGV[1])
local del_count = tonumber(ARGV[2])
if set_pair_count == nil or del_count == nil then
  return redis.error_reply("set_pair_count and del_count must be numbers")
end

local game_id = ARGV[argc - 2]
local channel = ARGV[argc - 1]
local ttl     = ARGV[argc]

local expected = 2 + (set_pair_count * 2) + del_count + 3
if argc ~= expected then
  return redis.error_reply("invalid arg count: expected " .. expected .. ", got " .. argc)
end

-- Build list of fields for HMGET
local fields = {}
local idx = 1
local set_start = 3
local set_end = set_start + (set_pair_count * 2) - 1
for i = set_start, set_end, 2 do
  fields[idx] = ARGV[i]
  idx = idx + 1
end
local del_start = set_end + 1
local del_end = del_start + del_count - 1
for i = del_start, del_end do
  fields[idx] = ARGV[i]
  idx = idx + 1
end

-- Fetch old values in one call
local oldvals = {}
if #fields > 0 then
  oldvals = redis.call("HMGET", key, unpack(fields))
end

-- Determine changes
local changed_members = {} -- flat list for HSET
local deleted_fields = {}
local changed_obj = {
  game_id = game_id
}
local changed_sets = 0
local changed_dels = 0

idx = 1
for i = set_start, set_end, 2 do
  local field = ARGV[i]
  local newv  = ARGV[i + 1]
  local oldv  = oldvals[idx]
  idx = idx + 1

  -- Compare raw strings; missing field counts as change
  if oldv ~= newv then
    table.insert(changed_members, field)
    table.insert(changed_members, newv)

    changed_obj[field] = tonumber(newv) or newv
    changed_sets = changed_sets + 1
  end
end

for i = del_start, del_end do
  local field = ARGV[i]
  local oldv  = oldvals[idx]
  idx = idx + 1

  -- Only delete if it existed
  if oldv ~= false and oldv ~= nil then
    table.insert(deleted_fields, field)
    changed_obj[field] = cjson.null
    changed_dels = changed_dels + 1
  end
end

-- Apply only changed fields
if changed_sets > 0 then
  redis.call("HSET", key, unpack(changed_members))
end

if changed_dels > 0 then
  redis.call("HDEL", key, unpack(deleted_fields))
end

-- Update TTL (independent of whether fields changed)
redis.call("EXPIRE", key, ttl)

-- Publish only if something changed
if (changed_sets + changed_dels) > 0 then
  local json = cjson.encode(changed_obj)
  redis.call("PUBLISH", channel, json)
end
