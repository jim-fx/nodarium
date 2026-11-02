;; A minimal node with two outputs (docs/ABI.md), used to test the wasm wrapper.
;; The first output is [argc], the second [length of the first input].
;;
;; Rebuild with: wat2wasm multi.wat -o multi.wasm (older wabt needs --enable-annotations)
(module
  (@custom "nodarium_definition" "{\"id\":\"test/abi/multi\",\"outputs\":[\"integer\",{\"type\":\"integer\",\"label\":\"length\"}],\"inputs\":{}}")

  (memory (export "memory") 1)

  (global $heap (mut i32) (i32.const 1024))

  (func $alloc (export "nodarium_alloc") (param $bytes i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap
      (i32.and
        (i32.add (i32.add (local.get $ptr) (local.get $bytes)) (i32.const 3))
        (i32.const -4)))
    (local.get $ptr))

  (func (export "nodarium_reset")
    (global.set $heap (i32.const 1024)))

  (func (export "nodarium_execute") (param $args i32) (param $argc i32) (result i32)
    (local $a i32)
    (local $b i32)
    (local $desc i32)

    (local.set $a (call $alloc (i32.const 4)))
    (i32.store (local.get $a) (local.get $argc))

    (local.set $b (call $alloc (i32.const 4)))
    (i32.store (local.get $b) (i32.load offset=4 (local.get $args)))

    ;; one [ptr, len] pair per output
    (local.set $desc (call $alloc (i32.const 16)))
    (i32.store (local.get $desc) (local.get $a))
    (i32.store offset=4 (local.get $desc) (i32.const 1))
    (i32.store offset=8 (local.get $desc) (local.get $b))
    (i32.store offset=12 (local.get $desc) (i32.const 1))
    (local.get $desc)))
