// The literal key forms in every place a key goes (DEPS-BUMP-3 step 5, the oxc 0.160.0 port:
// a non-computed string, numeric or BigInt key is its own PropertyKey variant there, a computed
// key an expression). Each list: the own keys in order, then the value of each key.
function show(label, o) {
  var keys = Reflect.ownKeys(o);
  var vals = [];
  for (var i = 0; i < keys.length; i++) vals.push(String(o[keys[i]]));
  print(label, keys.join(" "), "|", vals.join(" "));
}

var lit = { "a": 1, 1: 2, 1.5: 3, 1e21: 4, 0x10: 5, 1_000: 6, .5: 7, 1n: 8, 0x2n: 9 };
show("object", lit);

var dup = { 1: "num", "1": "str", 1n: "big", 0x1n: "hexbig", 1.0: "float" };
show("same key", dup);

var comp = { ["a"]: 1, [1]: 2, [1.5]: 3, [1e21]: 4, [0x10]: 5, [1_000]: 6, [.5]: 7, [1n]: 8, [0x2n]: 9 };
show("computed", comp);

class F {
  "a" = 1; 1 = 2; 1.5 = 3; 1e21 = 4; 0x10 = 5; 1_000 = 6; .5 = 7; 1n = 8; 0x2n = 9;
  static "a" = 10; static 1.5 = 30; static 1n = 80;
}
show("field", new F());
show("static field", F);

class M {
  "a"() { return 1; } 1() { return 2; } 1.5() { return 3; } 1e21() { return 4; }
  0x10() { return 5; } 1_000() { return 6; } .5() { return 7; } 1n() { return 8; }
  0x2n() { return 9; }
}
var names = [];
var mkeys = Object.getOwnPropertyNames(M.prototype);
for (var i = 0; i < mkeys.length; i++) {
  var k = mkeys[i];
  if (k !== "constructor") names.push(k + "=" + M.prototype[k].name + ":" + M.prototype[k]());
}
print("method", names.join(" "));

var src = { "a": 1, "1": 2, "1.5": 3, "1e+21": 4, "16": 5, "1000": 6, "0.5": 7, "2": 9 };
var { "a": pa, 1: p1, 1.5: p15, 1e21: pe, 0x10: px, 1_000: pk, .5: ph, 1n: pb, 0x2n: pbx } = src;
print("pattern", pa, p1, p15, pe, px, pk, ph, pb, pbx);
var { 1.5: q15, 0x10: qx, ...rest } = src;
print("pattern rest", q15, qx, Object.keys(rest).join(" "));
var t = {};
({ "a": t.a, 1.5: t.b, 1n: t.c, 0x10: t.d } = src);
print("assign pattern", t.a, t.b, t.c, t.d);

var g = {
  get "a"() { return 1; }, get 1() { return 2; }, get 1.5() { return 3; }, get 1e21() { return 4; },
  get 0x10() { return 5; }, get 1_000() { return 6; }, get .5() { return 7; }, get 1n() { return 8; },
  get 0x2n() { return 9; },
};
show("getter", g);
var gn = [];
var gkeys = Object.getOwnPropertyNames(g);
for (var i = 0; i < gkeys.length; i++) gn.push(Object.getOwnPropertyDescriptor(g, gkeys[i]).get.name);
print("getter name", gn.join(","));

class G {
  get 1.5() { return "g15"; } static get 1n() { return "g1n"; } set 0x10(v) { this.v = v; }
}
var gi = new G();
gi[16] = "set16";
print("class accessor", gi["1.5"], G[1], gi.v,
  Object.getOwnPropertyDescriptor(G.prototype, "16").set.name);
