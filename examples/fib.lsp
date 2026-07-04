#!/usr/bin/env niilisp
; Recursion, higher-order functions, and lists.

(define (fib n)
  (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))

(println "fib 0..10:")
(println (map fib (sequence 0 10)))

(define (fact n)
  (if (< n 2) 1 (* n (fact (- n 1)))))

(println "10! = " (fact 10))
