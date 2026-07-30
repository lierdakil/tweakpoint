local socket_path = noctalia.getConfig("socket") or "/tmp/tweakpoint.sock"

local function on_line(line)
  local data, err = noctalia.json.decode(line)
  if not data then
    noctalia.notifyError("Tweakpoint: JSON parse error: " .. tostring(err))
    return
  end
  if type(data) == "table" then
    barWidget.setText(data.text or "")
  end
end

local ok = noctalia.runStream(
  "tweakpoint-waybar " .. socket_path,
  on_line
)

if not ok then
  noctalia.notifyError("Tweakpoint: failed to start daemon")
end
