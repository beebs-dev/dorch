local leader = redis.call("HGET", KEYS[1], "leader")
if not leader then
  return nil
end
local ttl = tonumber(ARGV[1])
if ttl and ttl > 0 then
  redis.call("EXPIRE", KEYS[1], ttl)
  redis.call("EXPIRE", KEYS[2], ttl)
end
local name = redis.call("HGET", KEYS[1], "name")
local members = redis.call("SMEMBERS", KEYS[2])
local out = { leader, name or "" }
for i = 1, #members do
  out[#out + 1] = members[i]
end
return out
