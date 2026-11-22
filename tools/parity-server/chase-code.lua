local bots = game.ReplicatedStorage.Bots
local tpl = bots:FindFirstChild("Little Brother")
local pl = game.Players:GetPlayers()[1]
local char = pl.Character
local root = char:WaitForChild("HumanoidRootPart")
local bot = tpl:Clone()
bot.Parent = workspace
local br = bot:FindFirstChild("HumanoidRootPart") or bot.PrimaryPart
bot:SetPrimaryPartCFrame(root.CFrame * CFrame.new(0, 0, -25))
print("bot parts", #bot:GetChildren(), bot.Humanoid.Health, bot.Humanoid.WalkSpeed)
for i = 1, 20 do
	wait(1)
	local d = (br.Position - root.Position).magnitude
	print("t", i, string.format("bot %.1f %.1f %.1f dist %.1f", br.Position.X, br.Position.Y, br.Position.Z, d))
end
