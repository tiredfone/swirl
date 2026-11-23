import math,sys
items=[]
rid=[0]
def ref():
    rid[0]+=1
    return f"RBX{rid[0]}"
def rot(rx,ry,rz):
    cx,sx,cy,sy,cz,sz=math.cos(rx),math.sin(rx),math.cos(ry),math.sin(ry),math.cos(rz),math.sin(rz)
    return [cy*cz,-cy*sz,sy,cz*sx*sy+cx*sz,cx*cz-sx*sy*sz,-cy*sx,sx*sz-cx*cz*sy,cz*sx+cx*sy*sz,cx*cy]
def part(name,cls,pos,size,anchored=False,rt=(0,0,0),extra="",kids="",r=None):
    r=r or ref()
    R=rot(*rt)
    cf="".join(f"<{k}>{v}</{k}>" for k,v in zip(["X","Y","Z","R00","R01","R02","R10","R11","R12","R20","R21","R22"],list(pos)+R))
    return f'<Item class="{cls}" referent="{r}"><Properties><string name="Name">{name}</string><CoordinateFrame name="CFrame">{cf}</CoordinateFrame><Vector3 name="size"><X>{size[0]}</X><Y>{size[1]}</Y><Z>{size[2]}</Z></Vector3><bool name="Anchored">{"true" if anchored else "false"}</bool>{extra}</Properties>{kids}</Item>', r
out=[]
out.append(part("Baseplate","Part",(0,-1,0),(200,2,200),True)[0])
out.append(part("B1","Part",(3,1,0),(2,2,2),False)[0])
out.append(part("B2","Part",(-4,1.5,2),(3,3,3),False,(0,0.4,0))[0])
out.append(part("Anch","Part",(0,3,6),(2,2,2),True)[0])
out.append(part("Far","Part",(30,1,0),(2,2,2),False)[0])
a,ra=part("WA","Part",(0,1,-5),(4,1,2),False)
b,rb=part("WB","Part",(3,2,-5),(2,2,2),False)
w=f'<Item class="Weld" referent="{ref()}"><Properties><string name="Name">W</string><Ref name="Part0">{ra}</Ref><Ref name="Part1">{rb}</Ref><CoordinateFrame name="C0"><X>3</X><Y>1</Y><Z>0</Z><R00>1</R00><R01>0</R01><R02>0</R02><R10>0</R10><R11>1</R11><R12>0</R12><R20>0</R20><R21>0</R21><R22>1</R22></CoordinateFrame></Properties></Item>'
out.append(a); out.append(b); out.append(w)
src="wait(1) local e = Instance.new('Explosion') e.Position = Vector3.new(0,1,0) e.BlastRadius = 10 e.BlastPressure = 500000 e.Hit:connect(function(p,d) print('hit', p.Name, string.format('%.4f', d)) end) e.Parent = workspace wait(1) print('exists', e.Parent ~= nil) wait(2) print('gone', e.Parent == nil)"
out.append(f'<Item class="Script" referent="{ref()}"><Properties><string name="Name">Boom</string><ProtectedString name="Source">{src}</ProtectedString></Properties></Item>')
xml='<roblox version="4"><Item class="Workspace" referent="RBW"><Properties><string name="Name">Workspace</string></Properties>'+"".join(out)+'</Item></roblox>'
open(sys.argv[1],'w').write(xml)
