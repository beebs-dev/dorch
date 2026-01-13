-- KEYS[1] = hash key
-- ARGV = [field1, value1, field2, value2, ..., channel, ttl]
-- The last ARGV entry are the PubSub channel and the new TTL for the key.

local key = KEYS[1]
local argc = #ARGV
if argc < 2 then
  return redis.error_reply("missing channel and ttl arg")
end

local channel = ARGV[argc-1]
local ttl = ARGV[argc]
local pair_count = argc - 2

-- Must be an even number of args before the channel (field/value pairs).
if (pair_count % 2) ~= 0 then
  return redis.error_reply("field/value args must be even; channel and ttl must be last")
end

-- Update fields
for i = 1, pair_count, 2 do
  redis.call("HSET", key, ARGV[i], ARGV[i + 1])
end

-- Update TTL
redis.call("EXPIRE", key, ttl)

-- Read back and publish as JSON
local flat = redis.call("HGETALL", key)
local obj = {}
for i = 1, #flat, 2 do
  local k = flat[i]
  local v = flat[i + 1]
  obj[k] = tonumber(v) or v
end

local json = cjson.encode(obj)
redis.call("PUBLISH", channel, json)