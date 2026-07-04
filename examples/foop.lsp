#!/usr/bin/env niilisp
; FOOP: functional object-oriented programming with contexts (ADR-0010).

(new Class 'Point)

; Constructor via the default functor.
(define (Point:Point (x 0) (y 0))
  (list Point x y))

; Methods live in the class context; `self` is the target object.
(define (Point:move dx dy)
  (inc (self 1) dx)
  (inc (self 2) dy)
  (self))

(define (Point:show)
  (println "Point(" (self 1) ", " (self 2) ")"))

(set 'p (Point 3 4))
(:show p)
(:move p 10 20)   ; mutates p in place
(:show p)
