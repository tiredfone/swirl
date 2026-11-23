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
def mover(cls,props):
    return f'<Item class="{cls}" referent="{ref()}"><Properties><string name="Name">M</string>{props}</Properties></Item>'
v3=lambda n,x,y,z: f'<Vector3 name="{n}"><X>{x}</X><Y>{y}</Y><Z>{z}</Z></Vector3>'
thrust=mover("BodyThrust",v3("Force",0,6000,0)+v3("Location",1.5,0,0))
out.append(part("Thrusted","Part",(0,3,0),(4,2,2),False,(0.1,0.2,0.1),"",thrust)[0])
cfid="<CoordinateFrame name=\"CFrame\"><X>0</X><Y>0</Y><Z>0</Z><R00>0.5403</R00><R01>0</R01><R02>0.84147</R02><R10>0</R10><R11>1</R11><R12>0</R12><R20>-0.84147</R20><R21>0</R21><R22>0.5403</R22></CoordinateFrame>"
gyro=mover("BodyGyro",cfid+v3("MaxTorque",400000,400000,400000)+'<float name="P">3000</float><float name="D">500</float>')
out.append(part("Gyroed","Part",(10,6,0),(4,2,2),False,(0.6,0.0,0.3),"",gyro)[0])
gyro2=mover("BodyGyro",cfid+v3("MaxTorque",0,400000,0)+'<float name="P">1000</float><float name="D">100</float>')
out.append(part("Gyroed2","Part",(-10,6,0),(2,4,2),False,(0.2,0.4,0.0),"",gyro2)[0])
xml='<roblox version="4"><Item class="Workspace" referent="RBW"><Properties><string name="Name">Workspace</string></Properties>'+"".join(out)+'</Item></roblox>'
open(sys.argv[1],'w').write(xml)
