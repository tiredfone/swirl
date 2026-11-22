import math,sys
rid=[0]
def ref():
    rid[0]+=1
    return f"RBX{rid[0]}"
def rot(rx,ry,rz):
    cx,sx,cy,sy,cz,sz=math.cos(rx),math.sin(rx),math.cos(ry),math.sin(ry),math.cos(rz),math.sin(rz)
    return [cy*cz,-cy*sz,sy,cz*sx*sy+cx*sz,cx*cz-sx*sy*sz,-cy*sx,sx*sz-cx*cz*sy,cz*sx+cx*sy*sz,cx*cy]
def cfx(pos,r=None):
    R=r or [1,0,0,0,1,0,0,0,1]
    return "".join(f"<{k}>{v}</{k}>" for k,v in zip(["X","Y","Z","R00","R01","R02","R10","R11","R12","R20","R21","R22"],list(pos)+R))
def part(name,pos,size,anchored,extra="",r=None,kids=""):
    r=r or ref()
    return f'<Item class="Part" referent="{r}"><Properties><string name="Name">{name}</string><CoordinateFrame name="CFrame">{cfx(pos)}</CoordinateFrame><Vector3 name="size"><X>{size[0]}</X><Y>{size[1]}</Y><Z>{size[2]}</Z></Vector3><bool name="Anchored">{"true" if anchored else "false"}</bool>{extra}</Properties>{kids}</Item>',r
PI=math.pi
def A(x,y,z,rx,ry,rz):
    return cfx((x,y,z),rot(rx,ry,rz))
joints=[("Neck","Head",A(0,1,0,-PI/2,0,PI),A(0,-0.5,0,-PI/2,0,PI)),("Right Shoulder","Right Arm",A(1,0.5,0,0,PI/2,0),A(-0.5,0.5,0,0,PI/2,0)),("Left Shoulder","Left Arm",A(-1,0.5,0,0,-PI/2,0),A(0.5,0.5,0,0,-PI/2,0)),("Right Hip","Right Leg",A(1,-1,0,0,PI/2,0),A(0.5,1,0,0,PI/2,0)),("Left Hip","Left Leg",A(-1,-1,0,0,-PI/2,0),A(-0.5,1,0,0,-PI/2,0))]
specs={"Head":((0,4.5,0),(2,1,1)),"Torso":((0,3,0),(2,2,1)),"Left Arm":((-1.5,3,0),(1,2,1)),"Right Arm":((1.5,3,0),(1,2,1)),"Left Leg":((-0.5,1,0),(1,2,1)),"Right Leg":((0.5,1,0),(1,2,1))}
def npc(name,ox,oz):
    refs={}
    ps={}
    for n,(p,s) in specs.items():
        r=ref();refs[n]=r;ps[n]=(p,s)
    items=[]
    for n,(p,s) in ps.items():
        kids=""
        if n=="Torso":
            for jn,p1,c0,c1 in joints:
                kids+=f'<Item class="Motor6D" referent="{ref()}"><Properties><string name="Name">{jn}</string><Ref name="Part0">{refs["Torso"]}</Ref><Ref name="Part1">{refs[p1]}</Ref><CoordinateFrame name="C0">{c0}</CoordinateFrame><CoordinateFrame name="C1">{c1}</CoordinateFrame></Properties></Item>'
        it,_=part(n,(p[0]+ox,p[1],p[2]+oz),s,False,"",refs[n],kids)
        items.append(it)
    items.append(f'<Item class="Humanoid" referent="{ref()}"><Properties><string name="Name">Humanoid</string></Properties></Item>')
    return f'<Item class="Model" referent="{ref()}"><Properties><string name="Name">{name}</string></Properties>{"".join(items)}</Item>'
out=[part("Baseplate",(0,-1,0),(300,2,300),True)[0]]
out.append(part("Wall",(12,2,0),(1,4,30),True)[0])
out.append(part("Wall2",(0,2,12),(30,4,1),True)[0])
script='<Item class="Script" referent="RBS"><Properties><string name="Name">T</string><string name="Source">wait(1)\nlocal PFS = game:GetService("PathfindingService")\nlocal path = PFS:CreatePath({AgentRadius = 2, AgentHeight = 5, AgentCanJump = true})\npath:ComputeAsync(Vector3.new(-10, 3, 0), Vector3.new(30, 3, 5))\nprint("status", tostring(path.Status))\nlocal wps = path:GetWaypoints()\nprint("count", #wps)\nfor i, w in ipairs(wps) do print(i, string.format("%.2f %.2f %.2f", w.Position.X, w.Position.Y, w.Position.Z), tostring(w.Action)) end\nlocal r = workspace:FindPartOnRay(Ray.new(Vector3.new(0, 10, 0), Vector3.new(0, -20, 0)))\nprint("ray", r and r.Name)</string></Properties></Item>'
xml='<roblox version="4"><Item class="Workspace" referent="RBW"><Properties><string name="Name">Workspace</string></Properties>'+"".join(out)+script+'</Item></roblox>'
open(sys.argv[1],'w').write(xml)
