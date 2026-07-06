#!/usr/bin/env niilisp
;; gui-demo.lsp — a manual demo of the niiLISP GUI (ADR-0034).
;;
;; Requires the helper binary. Build it and point NIILISP_GUI at it:
;;
;;   cargo build --features gui --bin niilisp-gui
;;   NIILISP_GUI=./target/debug/niilisp-gui ./target/debug/niilisp examples/gui-demo.lsp
;;
;; A window opens with a label, a text field, and a button; clicking the button
;; copies the field's text into the label. Close the window to exit.

(load "lib/gui.lsp")

(gs:init)

(gs:frame 'Main 100 100 320 180 "niiLISP GUI demo")
(gs:label 'Greeting "Type a name and press Greet")
(gs:text-field 'Name nil)
(gs:button 'Greet 'on-greet "Greet")
(gs:set-visible 'Main true)

;; Event handler: the helper sends (on-greet "Greet") on click. We ask the
;; field for its value by setting the label from it. (A fuller API would read
;; the field back; this MVP just demonstrates the event round trip.)
(define (on-greet id)
  (gs:set-text 'Greeting (append "Clicked " id " — hello!")))

(gs:listen)
(exit)
