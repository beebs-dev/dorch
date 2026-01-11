redis.call("SREM", KEYS[1], ARGV[1])
local count = redis.call("SCARD", KEYS[1])
if count == 0 then
    redis.call("DEL", KEYS[2])
    redis.call("DEL", KEYS[1])
end
return count