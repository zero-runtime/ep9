# K2: simulate the f-design L1 CTOR-FIT rule on the census by_f counts (scratch tool).
import sys,re
GEN=8
def growth(j):
    # SLOTS growth at out-of-line index j: n = max(j+1, 2*have, 4); returns new capacity or 0
    if j==0: return 4
    if j>=4 and (j & (j-1))==0: return 2*j
    return 0
rows=[]
for l in open(sys.argv[1]):
    if 'census: classf ' not in l: continue
    m=re.search(r'classf (.*?) base=(\d+) names=(\d+) extra=(\d+) escaped=(\w+) bytecode=(\w+) today_cap=\[(.*?)\] by_f=\[(.*?)\] loads_by_f=\[(.*?)\]',l)
    name,base,names,extra,esc,bc,caps,byf,lbf=m.groups()
    base,names,extra=int(base),int(names),int(extra); esc=esc=='true'; bc=bc=='true'
    caps=[tuple(map(int,x.split(':'))) for x in caps.split()] if caps else []
    c0=max(caps,key=lambda t:t[1])[0] if caps else GEN
    byf=[int(x) for x in byf.split()] if byf else []
    lbf=[tuple(map(int,x.split('/'))) for x in lbf.split()] if lbf else []
    rows.append((name,base,names,extra,esc,bc,c0,byf,lbf))
def rule(names,extra,esc,bc,c0,k):
    if not bc: return c0
    n=names+min(extra,GEN)+(k if esc else 0)
    return min(n,255) if n>GEN else c0
def sim(byf,cap):
    cells=0;b=0
    for f,a in enumerate(byf):
        if f>=cap:
            n=growth(f-cap)
            if n: cells+=a; b+=a*(16+8*n)
    return cells,b
tot={}
out=[]
for k in (0,1,2):
    T=dict(cells0=0,bytes0=0,cells=0,bytes=0,objd=0,conv=0,outl=0,inl=0,inst=0)
    per=[]
    for name,base,names,extra,esc,bc,c0,byf,lbf in rows:
        if not byf: continue
        cap=rule(names,extra,esc,bc,c0,k)
        c_0,b_0=sim(byf,c0); c_1,b_1=sim(byf,cap)
        inst=byf[0]
        objd=inst*8*(cap-c0)
        conv=sum(lbf[f][1] for f in range(c0,min(cap,len(lbf))))
        T['cells0']+=c_0;T['bytes0']+=b_0;T['cells']+=c_1;T['bytes']+=b_1;T['objd']+=objd;T['conv']+=conv
        T['outl']+=sum(x[1] for x in lbf);T['inl']+=sum(x[0] for x in lbf);T['inst']+=inst
        per.append((c_0-c_1,name,c0,cap,inst,c_0,c_1,b_0-b_1,objd,conv))
    tot[k]=(T,sorted(per,reverse=True))
for k in (0,1,2):
    T,per=tot[k]
    net=T['bytes0']-T['bytes']-T['objd']
    print(f"K={k} SLOTS cells today={T['cells0']} rule={T['cells']} removed={T['cells0']-T['cells']} SLOTS bytes removed={T['bytes0']-T['bytes']} object bytes added={T['objd']} net bytes removed={net} out-of-line loads to in-object={T['conv']} of out-of-line={T['outl']} in-object={T['inl']}")
T,per=tot[0]
print("top classes K=0: name cap_today cap_rule instances slots_today slots_rule slots_bytes_removed obj_bytes_added oop_loads_converted")
for r in per[:20]:
    print(f"  {r[1]} {r[2]} {r[3]} {r[4]} {r[5]} {r[6]} {r[7]} {r[8]} {r[9]}")
for k in (1,2):
    T,per=tot[k]
    print(f"top classes K={k}:")
    for r in per[:8]:
        print(f"  {r[1]} {r[2]} {r[3]} {r[4]} {r[5]} {r[6]} {r[7]} {r[8]} {r[9]}")
