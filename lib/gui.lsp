;; gui.lsp — the gs: GUI API for niiLISP (ADR-0034).
;;
;; A thin sender layer over the `niilisp-gui` helper process: each gs: function
;; formats a command line (arbitrary text base64-encoded) and net-sends it to
;; the helper, which renders with fltk. Events arrive back as niiLISP source
;; lines that gs:listen / gs:check-event eval-string.
;;
;; The API mirrors newLISP-GS's gs: vocabulary closely so existing scripts run
;; with few edits, but makes no behaviour guarantee (a different toolkit does
;; not match Swing's pixels/layout).

(context 'gs)

;; ---- command formatting (pure; unit-tested) -----------------------------

;; Base64-encode a text argument (labels, titles) for a command token.
(define (gs:enc txt) (base64-enc (string txt)))

;; Join a command name and already-stringified tokens into a protocol line.
(define (gs:line)
  (join (map string (args)) " "))

;; ---- transport ----------------------------------------------------------

(define (gs:send line)
  (net-send gs:sock (append line "\n")))

;; Launch the helper and connect. gs:sock becomes the full-duplex socket.
(define (gs:init (port 64010))
  (set 'gs:lsock (net-listen port))
  (process (append (or (env "NIILISP_GUI") "niilisp-gui") " " (string port)))
  (set 'gs:sock (net-accept gs:lsock))
  (net-close gs:lsock)
  (!= gs:sock nil))

;; ---- widgets (each builds a line and sends it) --------------------------

(define (gs:frame-cmd id x y w h title)
  (gs:line "frame" id x y w h (gs:enc title)))
(define (gs:frame id x y w h title)
  (gs:send (gs:frame-cmd id x y w h title)))

;; gs:window is an alias for a top-level frame.
(define (gs:window id x y w h title)
  (gs:frame id x y w h title))

(define (gs:panel-cmd id)
  (gs:line "panel" id))
(define (gs:panel id)
  (gs:send (gs:panel-cmd id)))

(define (gs:button-cmd id action text)
  (gs:line "button" id action (gs:enc text)))
(define (gs:button id action text)
  (gs:send (gs:button-cmd id action text)))

(define (gs:label-cmd id text)
  (gs:line "label" id (gs:enc text)))
(define (gs:label id text)
  (gs:send (gs:label-cmd id text)))

(define (gs:text-field-cmd id action)
  (gs:line "text-field" id action))
(define (gs:text-field id action)
  (gs:send (gs:text-field-cmd id action)))

(define (gs:add-to-cmd parent)
  (gs:line "add-to" parent (join (map string (rest (args))) " ")))
(define (gs:add-to)
  (gs:send (join (map string (cons "add-to" (args))) " ")))

(define (gs:set-text-cmd id text)
  (gs:line "set-text" id (gs:enc text)))
(define (gs:set-text id text)
  (gs:send (gs:set-text-cmd id text)))

(define (gs:set-background-cmd id r g b)
  (gs:line "set-background" id r g b))
(define (gs:set-background id r g b)
  (gs:send (gs:set-background-cmd id r g b)))

(define (gs:set-visible-cmd id flag)
  (gs:line "set-visible" id (if flag 1 0)))
(define (gs:set-visible id flag)
  (gs:send (gs:set-visible-cmd id flag)))

;; ---- event loop ---------------------------------------------------------

;; Blocking loop: evaluate each incoming event line until the socket closes.
(define (gs:listen)
  (while (net-receive gs:sock gs:event 1000000 "\n")
    (eval-string gs:event)))

;; Non-blocking poll: run at most one pending event, waiting up to `us` micros.
(define (gs:check-event us)
  (when (net-select gs:sock "read" (/ (or us 0) 1000))
    (net-receive gs:sock gs:event 1000000 "\n")
    (eval-string gs:event))
  true)

(context MAIN)
