;; A minimal node with the pre-v1 exports, used to test the wasm wrapper's
;; legacy fallback. `__alloc` counts i32 words, `execute` receives all inputs
;; concatenated into one array and returns [len, ...that array].
;;
;; Rebuild with: wat2wasm --enable-annotations legacy.wat -o legacy.wasm
(module
  (@custom "nodarium_definition" "{\"id\":\"test/abi/legacy\",\"outputs\":[\"float\"],\"inputs\":{}}")

  (memory (export "memory") 1)

  (global $heap (mut i32) (i32.const 1024))

  (func $alloc (export "__alloc") (param $len i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.add (local.get $ptr) (i32.shl (local.get $len) (i32.const 2))))
    (local.get $ptr))

  (func (export "__free") (param $ptr i32) (param $len i32))

  (func (export "execute") (param $ptr i32) (param $len i32) (result i32)
    (local $out i32)
    (local $i i32)
    (local.set $out (call $alloc (i32.add (local.get $len) (i32.const 1))))
    (i32.store (local.get $out) (local.get $len))
    (block $done
      (loop $words
        (br_if $done (i32.ge_u (local.get $i) (local.get $len)))
        (i32.store
          (i32.add (local.get $out) (i32.shl (i32.add (local.get $i) (i32.const 1)) (i32.const 2)))
          (i32.load (i32.add (local.get $ptr) (i32.shl (local.get $i) (i32.const 2)))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $words)))
    (local.get $out)))
