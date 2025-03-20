(wrb-root u80 u120)

(define-constant VIEWPORT_STATUS u0)
(define-constant VIEWPORT_WIDGETS u1)
(define-constant VIEWPORT_CONSOLE u2)
(define-constant BLACK u0)
(define-constant RED (buff-to-uint-be 0xff0000))
(define-constant WHITE (buff-to-uint-be 0xffffff))
(define-constant GREEN (buff-to-uint-be 0x00ff00))
(define-constant YELLOW (buff-to-uint-be 0xffff00))

(wrb-viewport VIEWPORT_STATUS u0 u0 u120 u60)
(wrb-viewport VIEWPORT_WIDGETS u0 u60 u120 u60)

;;;;;;;;;;;;;;;;;;;;;;;; Console Log ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

(define-constant CONSOLE_LINE_LEN u512)
(define-constant CONSOLE_LINE_COUNT u20) 

(wrb-viewport VIEWPORT_CONSOLE u58 u0 CONSOLE_LINE_COUNT CONSOLE_LINE_LEN)
(wrb-set-static-txt-colors VIEWPORT_CONSOLE GREEN BLACK)
(wrb-set-txt-colors VIEWPORT_CONSOLE GREEN BLACK)

(define-data-var console-lineno uint u0)
(define-data-var console-buffer (string-utf8 8000) u"")
(define-map console-lines
    ;; line number
    uint
    ;; length of line added here
    uint)

(define-private (debug! (msg (string-utf8 12200)))
    (console-log! u"DEBG" msg))

(define-private (info! (msg (string-utf8 12200)))
    (console-log! u"INFO" msg))

(define-private (warn! (msg (string-utf8 12200)))
    (console-log! u"WARN" msg))

(define-private (error! (msg (string-utf8 12200)))
    (console-log! u"ERRO" msg))

(define-private (console-log! (level (string-utf8 4)) (msg (string-utf8 12200)))
    (let (
        (trunc-msg (concat
            ;; truncate message to CONSOLE_LINE_LEN - 1 chars and add '\n' to the end
            (if (< (len msg) (- CONSOLE_LINE_LEN u1))
               (unwrap-panic (as-max-len? msg u511))
               (unwrap-panic (as-max-len? (unwrap-panic (slice? msg u0 (- CONSOLE_LINE_LEN u1))) u511)))
            u"\n"))
        (lineno (var-get console-lineno))
        (last-line-len (default-to u0 (map-get? console-lines (mod lineno CONSOLE_LINE_COUNT))))
        (console-buff (var-get console-buffer))
        (next-console-buff (default-to u"(overflow)"
            (as-max-len?
            (concat
                (if (>= lineno CONSOLE_LINE_COUNT)
                    (default-to u"(slice failed)" (slice? console-buff last-line-len (len console-buff)))
                    console-buff)
                trunc-msg)
            u8000)))
    )
    (var-set console-buffer next-console-buff)
    (map-set console-lines (mod lineno CONSOLE_LINE_COUNT) (len trunc-msg))
    (var-set console-lineno (+ u1 lineno))))

(define-private (show-console-log)
    (let (
        (lineno (var-get console-lineno))
        (row-offset
            (if (< lineno CONSOLE_LINE_COUNT)
                (- CONSOLE_LINE_COUNT lineno)
                u0))
        (console-buff (var-get console-buffer))
     )
     (wrb-viewport-clear VIEWPORT_CONSOLE)
     (wrb-print-immediate VIEWPORT_CONSOLE (some { row: row-offset, col: u0 }) BLACK GREEN console-buff)))

;; TODO: clear line lens
(define-private (clear-console-log)
    (begin
        (var-set console-buffer u"")
        (var-set console-lineno u0)))

(define-private (errmsg (str (string-ascii 512)))
    (default-to "(message too long)" (as-max-len? str u512)))

;; Helper for printing out wrblib API error messages
(define-private (error-ascii! (str (string-ascii 512)))
    (begin
        (error! (default-to u"(message too long)"
                    (as-max-len? (match (wrb-string-ascii-to-string-utf8? str)
                        ok-res ok-res
                        err-res u"(message too long)")
                    u12200)))
        (default-to "(message too long)" (as-max-len? str u512))))

;; Flag to indicate if the console is visible
(define-data-var console-visible bool true)

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; End ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

(wrb-static-txt-immediate VIEWPORT_WIDGETS u0 u0 BLACK WHITE u"Widgets demo")

(define-constant BUTTON_OK
    (wrb-button VIEWPORT_WIDGETS u2 u0 u"  OK  "))
(define-constant BUTTON_CANCEL
    (wrb-button VIEWPORT_WIDGETS u2 u10 u"Cancel"))

(define-constant TOGGLE_CONSOLE
    (wrb-button VIEWPORT_WIDGETS u4 u0 u"Toggle console"))

(define-constant CHECKBOX_1
    (wrb-checkbox VIEWPORT_WIDGETS u6 u0 (list
        {
            text: u"Wrb is not the Web",
            selected: false
        }
        {
            text: u"Wrb is built on Bitcoin",
            selected: false
        }
        {
            text: u"Wrb is built on Stacks",
            selected: true
        })))

(define-constant TEXTLINE_1
    (wrb-textline VIEWPORT_WIDGETS u12 u0 u60 u"Initial text"))

(define-constant TEXTAREA_1
    (wrb-textarea VIEWPORT_WIDGETS u17 u0 u3 u60 (* u2 u3 u60) u"Initial text"))

(define-data-var event-count uint u0)

(define-private (inner-setup-wrbpod)
    (let (
        (appname (wrb-get-app-name))
        (wrbpod-session (unwrap! (wrbpod-open (wrbpod-default))
            (err (errmsg "setup-wrbpod: Failed to open default wrbpod"))))
        (num-slots (unwrap! (wrbpod-get-num-slots wrbpod-session { name: (get name appname), namespace: (get namespace appname) })
            (err (errmsg "setup-wrbpod: Failed to get number of slots for default wrbpod session"))))
    )
    (if (is-eq num-slots u0)
        (begin 
           (unwrap! (wrbpod-alloc-slots wrbpod-session u1)
               (err (errmsg "setup-wrbpod: Failed to allocate slots to default wrbpod")))
           (ok true))
        (ok true))))

(define-private (setup-wrbpod)
    (let (
       (result (inner-setup-wrbpod))
    )
    (match result
        ok-res (ok ok-res)
        err-res (err { code: u1, message: (error-ascii! err-res) }))))

(define-private (inner-get-count-from-wrbpod)
    (let (
        (wrbpod-session (unwrap! (wrbpod-open (wrbpod-default))
            (err (errmsg "get-count-from-wrbpod: Failed to open default wrbpod"))))
    )
    (try! (match (wrbpod-fetch-slot wrbpod-session u0)
        slot (ok true)
        err-resp (err (errmsg (default-to "get-count-from-wrbpod: failed to fetch slot u0 from session" (as-max-len? (concat "get-count-from-wrbpod: failed to fetch slot u0 from session: " (get message err-resp)) u512))))))
    (let (
        (count-buff (unwrap! (wrbpod-get-slice wrbpod-session u0 u0)
            (err (errmsg "get-count-from-wrbpod: Failed to get slice 0 of slot 0"))))
        (count (unwrap! (from-consensus-buff? uint count-buff)
            (err (errmsg "get-count-from-wrbpod: Failed to decode count"))))
    )
    (ok count))))

(define-private (get-count-from-wrbpod)
    (let (
        (result (inner-get-count-from-wrbpod))
    )
    (match result
        ok-res (ok ok-res)
        err-res (err { code: u1, message: (error-ascii! err-res) }))))

(define-private (inner-save-count-to-wrbpod (count uint))
    (let (
        (wrbpod-session (unwrap! (wrbpod-open (wrbpod-default))
            (err (errmsg "save-count-to-wrbpod: Failed to open default wrbpod"))))
    )
    (try! (match (wrbpod-fetch-slot wrbpod-session u0)
        slot (ok true)
        err-resp (err (errmsg (default-to "save-count-to-wrbpod: failed to fetch slot u0 from session" (as-max-len? (concat "save-count-to-wrbpod: failed to fetch slot u0 from session: " (get message err-resp)) u512))))))
    (unwrap! (wrbpod-put-slice wrbpod-session u0 u0 (unwrap-panic (to-consensus-buff? count)))
        (err (errmsg "save-count-to-wrbpod: Failed to store `count` to slice 0 of slot 0 of default wrbpod")))
    (unwrap! (wrbpod-sync-slot wrbpod-session u0)
        (err (errmsg "save-count-to-wrbpod: Failed to sync slot 0 of default wrbpod")))
    (ok true)))

(define-private (save-count-to-wrbpod (count uint))
    (let (
        (result (inner-save-count-to-wrbpod count))
    )
    (match result
        ok-res (ok ok-res)
        err-res (err { code: u1, message: (error-ascii! err-res) }))))

;; Handle the WRB_EVENT_OPEN case
(define-data-var handled-page-open bool false)
(define-private (handle-page-open (event-type uint))
    (begin
        (if (is-eq event-type WRB_EVENT_OPEN)
            (var-set handled-page-open false)
            true)

        (if (var-get handled-page-open)
            true
            (begin
                (clear-console-log)
                (debug! u"Handle WRB_EVENT_OPEN: setup-wrbpod")
                (match (setup-wrbpod)
                    ok-res (begin
                        (var-set handled-page-open true) 
                        (debug! u"Opened page!")
                        true)
                    err-res (begin
                        (debug! (concat u"Failed to run `setup-wrbpod`: code " (int-to-utf8 (get code err-res))))
                        false))))))

;; Handle the WRB_EVENT_RESIZE case
(define-private (handle-page-resize (event-type uint) (event-payload (buff 1024)))
    (if (is-eq event-type WRB_EVENT_RESIZE)
        (let (
            (resize-dims (unwrap! (from-consensus-buff? { rows: uint, cols: uint } event-payload)
                (err (errmsg "Failed to decode WRB_EVENT_RESIZE payload"))))
        )
        (debug! (concat u"Got resize event: " (concat u"rows = " (concat (int-to-utf8 (get rows resize-dims)) (concat u", cols = " (int-to-utf8 (get cols resize-dims)))))))
        (ok true))
        (ok true)))

;; Handle button press to TOGGLE_CONSOLE
(define-private (handle-toggle-console (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (if (and (is-eq element-type WRB_UI_TYPE_BUTTON) (is-eq element-id TOGGLE_CONSOLE) (is-eq event-type WRB_EVENT_UI))
        (let (
            (is-visible (var-get console-visible))
        )
        (var-set console-visible (not is-visible))
        (wrb-viewport-set-visible VIEWPORT_CONSOLE (not is-visible))
        (debug! u"Toggled console"))
        true))
             
;; Handle events
(define-private (handle-events (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (begin
        (debug! (concat
            u"Got event #" (concat
            (concat (int-to-utf8 (var-get event-count)) u": type: ") (concat
            (int-to-utf8 event-type) (concat
            u", element-type: " (concat
            (int-to-utf8 element-type) (concat
            u", element-id: " (int-to-utf8 element-id))))))))

        (handle-page-open event-type)
        (handle-toggle-console element-type element-id event-type event-payload)
        (match (handle-page-resize event-type event-payload)
            ok-res ""
            err-res (error-ascii! err-res))  
        true))

(define-public (main (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (let (
        (events (var-get event-count))
        (wrbpod-count (match (get-count-from-wrbpod)
            value value
            error u111111111))
    )
    (wrb-viewport-clear VIEWPORT_STATUS)
    (wrb-txt-immediate VIEWPORT_STATUS u0 u0 BLACK WHITE (concat u"Ran event loop " (concat (int-to-utf8 events) u" time(s)" )))
    (wrb-txt-immediate VIEWPORT_STATUS u1 u0 BLACK WHITE (concat u"Count from wrbpod: " (int-to-utf8 wrbpod-count)))
    
    (handle-events element-type element-id event-type event-payload)
    (show-console-log)

    (var-set event-count (+ u1 events))

    (try! (save-count-to-wrbpod events))
    (ok true)))

(wrb-event-loop "main")
(wrb-event-loop-time u1000)
(wrb-event-subscribe WRB_EVENT_CLOSE)
(wrb-event-subscribe WRB_EVENT_RESIZE)
(wrb-event-subscribe WRB_EVENT_TIMER)
(wrb-event-subscribe WRB_EVENT_UI)

