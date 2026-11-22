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
out.append(part("Block","Part",(0,10,0),(4,2,2),False,(0.3,0.5,0.2))[0])
out.append(part("Ball","Part",(8,14,2),(3,3,3),False,(0,0,0),'<token name="shape">0</token>')[0])
out.append(part("Cyl","Part",(-8,12,-3),(2,4,4),False,(0.1,0,0.4),'<token name="shape">2</token>')[0])
out.append(part("Wedge","WedgePart",(4,18,-6),(4,3,5),False,(0.2,1.0,0.5))[0])
a,ra=part("WeldA","Part",(-20,15,0),(4,1,2),False,(0,0,0))
b,rb=part("WeldB","Part",(-16,16,0),(2,2,2),False,(0.2,0.3,0.1))
w=f'<Item class="Weld" referent="{ref()}"><Properties><string name="Name">W</string><Ref name="Part0">{ra}</Ref><Ref name="Part1">{rb}</Ref><CoordinateFrame name="C0"><X>3</X><Y>1</Y><Z>0</Z><R00>1</R00><R01>0</R01><R02>0</R02><R10>0</R10><R11>1</R11><R12>0</R12><R20>0</R20><R21>0</R21><R22>1</R22></CoordinateFrame></Properties></Item>'
a=a.replace("</Item>",w+"</Item>") if False else a
out.append(a); out.append(b)
out.append(w.replace('referent="RBX','referent="RBY'))
for i in range(6):
    out.append(part(f"T{i}","Part",(20,1.0+i*2.0+0.05*i,5),(4,2,4),False,(0,0.02*i,0))[0])
out.append(part("Slider","Part",(30,3,-10),(6,1,6),False,(0,0,0.5),'<float name="Friction">0.1</float>')[0])
for i in range(4):
    out.append(part(f"Pile{i}","Part",(-30+i*0.7,4+i*3.1,10+i*0.4),(3,3,3),False,(0.5*i,0.3*i,0.2*i))[0])
xml='<roblox version="4"><Item class="Workspace" referent="RBW"><Properties><string name="Name">Workspace</string></Properties>'+"".join(out)+'</Item></roblox>'
open(sys.argv[1],'w').write(xml)
