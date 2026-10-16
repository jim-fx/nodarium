;; A minimal node that implements the ABI v1 (docs/ABI.md), used to test the
;; wasm wrapper. It returns [argc, ...all input words], so a test can check
;; exactly what arrived. Without inputs it panics.
;;
;; Rebuild with: wat2wasm --enable-annotations v1.wat -o v1.wasm
(module
  (@custom "nodarium_definition" "{\"id\":\"test/abi/v1\",\"outputs\":[\"float\"],\"inputs\":{}}")

  (import "env" "nodarium_panic" (func $panic (param i32 i32)))

  (memory (export "memory") 1)
  (data (i32.const 16) "no inputs")

  ;; bump allocator, everything below 1024 is reserved for static data
  (global $heap (mut i32) (i32.const 1024))

  (func $alloc (export "nodarium_alloc") (param $bytes i32) (result i32)
    (local $ptr i32)
    (local $needed i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap
      (i32.and
        (i32.add (i32.add (local.get $ptr) (local.get $bytes)) (i32.const 3))
        (i32.const -4)))
    ;; grow the memory if the allocation doesn't fit
    (local.set $needed
      (i32.sub
        (i32.shr_u (i32.add (global.get $heap) (i32.const 65535)) (i32.const 16))
        (memory.size)))
    (if (i32.gt_s (local.get $needed) (i32.const 0))
      (then (drop (memory.grow (local.get $needed)))))
    (local.get $ptr))

  (func (export "nodarium_reset")
    (global.set $heap (i32.const 1024)))

  (func (export "nodarium_execute") (param $args i32) (param $argc i32) (result i32)
    (local $total i32)
    (local $i i32)
    (local $j i32)
    (local $ptr i32)
    (local $len i32)
    (local $out i32)
    (local $dst i32)
    (local $desc i32)

    (if (i32.eqz (local.get $argc))
      (then
        (call $panic (i32.const 16) (i32.const 9))
        (unreachable)))

    ;; count the words of all inputs
    (local.set $i (i32.const 0))
    (loop $count
      (local.set $total
        (i32.add (local.get $total)
          (i32.load offset=4 (i32.add (local.get $args) (i32.shl (local.get $i) (i32.const 3))))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $count (i32.lt_u (local.get $i) (local.get $argc))))

    ;; [argc, ...inputs]
    (local.set $out (call $alloc (i32.shl (i32.add (local.get $total) (i32.const 1)) (i32.const 2))))
    (i32.store (local.get $out) (local.get $argc))
    (local.set $dst (i32.add (local.get $out) (i32.const 4)))

    (local.set $i (i32.const 0))
    (loop $inputs
      (local.set $ptr (i32.load (i32.add (local.get $args) (i32.shl (local.get $i) (i32.const 3)))))
      (local.set $len
        (i32.load offset=4 (i32.add (local.get $args) (i32.shl (local.get $i) (i32.const 3)))))
      (local.set $j (i32.const 0))
      (block $done
        (loop $words
          (br_if $done (i32.ge_u (local.get $j) (local.get $len)))
          (i32.store (local.get $dst)
            (i32.load (i32.add (local.get $ptr) (i32.shl (local.get $j) (i32.const 2)))))
          (local.set $dst (i32.add (local.get $dst) (i32.const 4)))
          (local.set $j (i32.add (local.get $j) (i32.const 1)))
          (br $words)))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $inputs (i32.lt_u (local.get $i) (local.get $argc))))

    ;; the [ptr, len] descriptor of the result
    (local.set $desc (call $alloc (i32.const 8)))
    (i32.store (local.get $desc) (local.get $out))
    (i32.store offset=4 (local.get $desc) (i32.add (local.get $total) (i32.const 1)))
    (local.get $desc)))
