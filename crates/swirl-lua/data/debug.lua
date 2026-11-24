
local R = ...
local base, sandbox = R.base, R.sandbox
local D = { bps = {}, mode = "run", depth = 0, watches = {}, waiting = nil }
local dlib = rawget(_G, "__RTDEBUG") or {}
local getinfo, getlocal = dlib.getinfo, dlib.getlocal
if not getinfo then
  getinfo = function() return nil end
  getlocal = function() return nil end
end
local function depthOf()
  local d = 0
  while getinfo(d + 3, "l") do d = d + 1 end
  return d
end
local function show(v)
  local t = type(v)
  if t == "string" then return string.format("%q", #v > 200 and v:sub(1, 200) .. "..." or v) end
  local ok, s = pcall(tostring, v)
  return ok and s or ("<" .. t .. ">")
end
local function collect(level)
  local locals, names = {}, {}
  local i = 1
  while true do
    local k, v = getlocal(level, i)
    if not k then break end
    if k:sub(1, 1) ~= "(" then
      names[k] = v
      locals[#locals + 1] = { k, show(v), typeof and typeof(v) or type(v) }
    end
    i = i + 1
  end
  return locals, names
end
local function stack(level)
  local out = {}
  local l = level
  while #out < 30 do
    local info = getinfo(l, "Sln")
    if not info then break end
    if info.what ~= "C" then out[#out + 1] = { info.short_src or "?", info.currentline or 0, info.name or "" } end
    l = l + 1
  end
  return out
end
local function evalWatches(names, env)
  local out = {}
  for i, expr in ipairs(D.watches) do
    local f, err = loadstring("return " .. expr, "=watch")
    local val
    if f then
      setfenv(f, setmetatable({}, { __index = function(_, k) local v = names[k] if v ~= nil then return v end return env[k] end }))
      local ok, r = pcall(f)
      val = ok and show(r) or ("error: " .. tostring(r))
    else
      val = "error: " .. tostring(err)
    end
    out[i] = { expr, val }
  end
  return out
end
local function enc(v)
  local t = type(v)
  if t == "table" then
    local parts, isArr = {}, #v > 0 or next(v) == nil
    if isArr then
      for i = 1, #v do parts[#parts + 1] = enc(v[i]) end
      return "[" .. table.concat(parts, ",") .. "]"
    end
    for k, x in pairs(v) do parts[#parts + 1] = string.format("%q", tostring(k)) .. ":" .. enc(x) end
    return "{" .. table.concat(parts, ",") .. "}"
  elseif t == "number" then return tostring(v)
  elseif t == "boolean" then return tostring(v)
  else return (string.format("%q", tostring(v)):gsub("\\\n", "\\n")) end
end
local function dbg(sid, line)
  local b = D.bps[sid]
  local hit = b and b[line]
  local d
  if not hit and D.mode ~= "run" then
    d = depthOf()
    if D.mode == "into" then hit = true
    elseif D.mode == "over" then hit = d <= D.depth
    elseif D.mode == "out" then hit = d < D.depth end
  end
  if not hit then return end
  d = d or depthOf()
  local locals, names = collect(3)
  local env = getfenv(2)
  local info = { sid = sid, line = line, locals = locals, stack = stack(3), watches = evalWatches(names, env) }
  local send = rawget(_G, "__DBGSEND")
  if send then send(enc(info)) end
  local ev = sandbox.Instance.new("BindableEvent")
  D.waiting = ev
  D.depth = d
  D.mode = "run"
  ev.Event:Wait()
end
base.__dbg = dbg
sandbox.__dbg = dbg
rawset(_G, "__DBGCMD", function(cmd, a, b)
  if cmd == "bps" then
    local t = {}
    for sid, list in string.gmatch(a or "", "([^;]+)=([^;]*)") do
      local s = {}
      for n in string.gmatch(list, "%d+") do s[tonumber(n)] = true end
      t[sid] = s
    end
    D.bps = t
  elseif cmd == "watches" then
    local w = {}
    for e in string.gmatch(a or "", "([^\n]+)") do w[#w + 1] = e end
    D.watches = w
  elseif cmd == "pause" then
    D.mode = "into"
  else
    D.mode = cmd == "continue" and "run" or cmd
    local ev = D.waiting
    D.waiting = nil
    if ev then ev:Fire() end
  end
end)
