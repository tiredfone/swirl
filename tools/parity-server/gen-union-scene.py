import struct,base64,sys,math
KEY=[86,46,110,88,49,32,48,4,52,105,12,119,12,1,94,0,26,96,55,105,29,82,43,7,79,36,89,101,83,4,122]
def csg(verts,idx):
    d=bytearray(b'CSGMDL')
    d+=struct.pack('<i',2)
    d+=bytes(42-len(d))
    d+=struct.pack('<ii',len(verts),24)
    for v in verts:
        d+=struct.pack('<ffffff',*v,0,1,0)
    d+=struct.pack('<i',len(idx))
    for i in idx: d+=struct.pack('<i',i)
    return bytes(b^KEY[i%len(KEY)] for i,b in enumerate(d))
v=[(-.5,-.5,-.5),(.5,-.5,-.5),(-.5,-.5,.5),(.5,-.5,.5),(-.5,.5,.5),(.5,.5,.5)]
idx=[0,1,3, 0,3,2, 2,3,5, 2,5,4, 0,4,5, 0,5,1, 0,2,4, 1,5,3]
xml='<roblox version="4"><Item class="UnionOperation" referent="U"><Properties><BinaryString name="MeshData">'+base64.b64encode(csg(v,idx)).decode()+'</BinaryString></Properties></Item></roblox>'
open(sys.argv[1],'w').write(xml)
