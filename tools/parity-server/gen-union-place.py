import sys,math
rid=[0]
def ref():
    rid[0]+=1
    return f"RBX{rid[0]}"
def cf(pos): return "".join(f"<{k}>{v}</{k}>" for k,v in zip(["X","Y","Z","R00","R01","R02","R10","R11","R12","R20","R21","R22"],list(pos)+[1,0,0,0,1,0,0,0,1]))
def part(cls,name,pos,size,anchored,extra=""):
    return f'<Item class="{cls}" referent="{ref()}"><Properties><string name="Name">{name}</string><CoordinateFrame name="CFrame">{cf(pos)}</CoordinateFrame><Vector3 name="size"><X>{size[0]}</X><Y>{size[1]}</Y><Z>{size[2]}</Z></Vector3><bool name="Anchored">{"true" if anchored else "false"}</bool>{extra}</Properties></Item>'
asset='<Content name="AssetId"><url>rbxassetid://9500</url></Content><Vector3 name="InitialSize"><X>1</X><Y>1</Y><Z>1</Z></Vector3>'
out=[part("Part","Baseplate",(0,-1,0),(200,2,200),True)]
out.append(part("UnionOperation","Ramp",(0,5,0),(20,10,20),True,asset))
out.append(part("UnionOperation","Box",(0,16,6),(4,4,4),False,asset))
out.append(part("Part","Ball",(-2,22,5),(3,3,3),False,'<token name="shape">0</token>'))
out.append(part("Part","Block",(3,25,5),(2,2,2),False))
src="wait(8) print(workspace.Ball.Position.Y, workspace.Block.Position.Y)"
out.append(f'<Item class="Script" referent="{ref()}"><Properties><string name="Name">T</string><string name="Source">{src}</string></Properties></Item>')
open(sys.argv[1],'w').write('<roblox version="4"><Item class="Workspace" referent="RBW"><Properties><string name="Name">Workspace</string></Properties>'+"".join(out)+'</Item></roblox>')
