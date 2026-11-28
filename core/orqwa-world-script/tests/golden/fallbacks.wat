;; A module answering every tick with the same output. Frozen: do not change.
(module
  (memory (export "memory") 4)
  (data (i32.const 16) "{\22bodies\22:[{\22id\22:\22b\22,\22kind\22:\22towed\22,\22riders\22:[0],\22fallback\22:{\22id\22:\22b\22,\22kind\22:\22scripted\22,\22riders\22:[0],\22speed_mps\22:3.0}},{\22id\22:\22c\22,\22kind\22:\22towed\22,\22riders\22:[1]}],\22trainer\22:[{\22slot\22:0,\22kind\22:\22slope\22,\22fallback\22:{\22slot\22:0,\22kind\22:\22grade\22,\22grade\22:0.02}},{\22slot\22:1,\22kind\22:\22slope\22}],\22scene\22:{\22nodes\22:[{\22id\22:\22n\22,\22model\22:\22figure\22,\22anchor\22:{\22lap\22:2},\22fallback\22:{\22id\22:\22n\22,\22model\22:\22figure\22,\22anchor\22:{\22route_m\22:50.0}}},{\22id\22:\22m\22,\22model\22:\22figure\22,\22anchor\22:{}}]},\22ui\22:[{\22id\22:\22p\22,\22slot\22:\22corner\22,\22items\22:[],\22fallback\22:{\22id\22:\22p\22,\22slot\22:\22top\22,\22items\22:[{\22kind\22:\22chart\22,\22fallback\22:{\22kind\22:\22text\22,\22text\22:{\22\22:\22chart\22}}},{\22kind\22:\22chart\22}]}}],\22future\22:{\22anything\22:true}}")
  (func (export "alloc") (param i32) (result i32) (i32.const 65536))
  (func (export "tick") (param i32 i32) (result i64)
    (i64.or (i64.const 16) (i64.shl (i64.const 643) (i64.const 32)))))
