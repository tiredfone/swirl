local pl = game.Players:GetPlayers()[1]
local ch = pl.Character
local hum = ch:FindFirstChild("Humanoid")
print("hip", hum.HipHeight, hum.RigType)
local names = {}
for _, p in ipairs(ch:GetChildren()) do
  if p:IsA("BasePart") then
    local c = p.CFrame
    print(p.Name, string.format("%.3f %.3f %.3f | %.3f %.3f %.3f | %.3f %.3f %.3f", p.Size.X, p.Size.Y, p.Size.Z, c.X, c.Y, c.Z, c:toEulerAnglesXYZ()))
    for _, d in ipairs(p:GetChildren()) do
      if d:IsA("Motor6D") then
        local a, b = d.C0, d.C1
        print(" j", d.Name, string.format("%.3f %.3f %.3f | %.3f %.3f %.3f", a.X, a.Y, a.Z, b.X, b.Y, b.Z))
      end
    end
  end
end
for _, d in ipairs(hum:GetChildren()) do print("hv", d.Name, d.ClassName, d:IsA("NumberValue") and d.Value or "") end
for _, d in ipairs(ch:GetChildren()) do if not d:IsA("BasePart") then print("child", d.Name, d.ClassName) end end
