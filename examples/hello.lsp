#!/usr/bin/env niilisp
; Basic output and arithmetic.

(println "Hello, niiLISP!")
(println "1 + 2 + 3 = " (+ 1 2 3))
(println "10 / 3   = " (/ 10 3) " (integer)")
(println "10 / 3   = " (div 10 3) " (float)")
