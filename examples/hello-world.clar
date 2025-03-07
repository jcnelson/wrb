(wrb-root u120 u120)

(define-constant VIEWPORT_STATUS u0)
(define-constant VIEWPORT_WIDGETS u1)
(define-constant VIEWPORT_ERROR u2)
(define-constant BLACK u0)
(define-constant RED (buff-to-uint-be 0xff0000))
(define-constant WHITE (buff-to-uint-be 0xffffff))

(wrb-viewport VIEWPORT_STATUS u0 u0 u120 u60)
(wrb-viewport VIEWPORT_WIDGETS u0 u60 u120 u60)
(wrb-viewport VIEWPORT_ERROR u60 u0 u40 u120)

(wrb-static-txt VIEWPORT_WIDGETS u0 u0 BLACK WHITE u"Widgets demo")

(define-constant BUTTON_OK
    (wrb-button VIEWPORT_WIDGETS u2 u0 u"  OK  "))
(define-constant BUTTON_CANCEL
    (wrb-button VIEWPORT_WIDGETS u2 u10 u"Cancel"))

(define-constant CHECKBOX_1
    (wrb-checkbox VIEWPORT_WIDGETS u4 u0 (list
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
    (wrb-textline VIEWPORT_WIDGETS u10 u0 u60 u"Initial text"))

(define-constant TEXTAREA_1
    (wrb-textarea VIEWPORT_WIDGETS u15 u0 u3 u60 (* u2 u3 u60) u"Initial text"))

(define-private (errmsg (str (string-ascii 512)))
    (unwrap-panic (as-max-len? str u512)))

(define-data-var err-countdown uint u0)
(define-data-var last-err-msg (string-utf8 12800) u"")

(define-private (display-errmsg (msg (string-ascii 512)))
    (let (
        (err-utf8
            (unwrap-panic (as-max-len?
                (unwrap-panic (wrb-string-ascii-to-string-utf8? msg))
                u12800)))
    )
        (unwrap-panic (wrb-viewport-clear VIEWPORT_ERROR))
        (unwrap-panic (wrb-txt VIEWPORT_ERROR u0 u0 RED WHITE u"   "))
        (unwrap-panic (wrb-txt VIEWPORT_ERROR u0 u3 RED WHITE err-utf8))
        (var-set err-countdown u5)
        (var-set last-err-msg err-utf8)
        (unwrap-panic (as-max-len? msg u512))
    ))

(define-private (update-errmsg-counter (err-ct uint))
    (if (< u0 err-ct)
        (begin
            (unwrap-panic (wrb-txt VIEWPORT_ERROR u0 u0 RED WHITE u"   "))
            (unwrap-panic (wrb-txt VIEWPORT_ERROR u0 u0 RED WHITE (int-to-utf8 err-ct)))
            (unwrap-panic (wrb-txt VIEWPORT_ERROR u0 u3 RED WHITE (var-get last-err-msg)))
            (var-set err-countdown (- err-ct u1))
            true)
        true))

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
        err-res (err { code: u1, message: (display-errmsg err-res) }))))

(define-private (inner-get-count-from-wrbpod)
    (let (
        (wrbpod-session (unwrap! (wrbpod-open (wrbpod-default))
            (err (errmsg "get-count-from-wrbpod: Failed to open default wrbpod"))))
    )
    ;; TODO: error out here on appropriate error
    (match (wrbpod-fetch-slot wrbpod-session u0)
        slot true
        err-txt false)
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
        err-res (err { code: u1, message: (display-errmsg err-res) }))))

(define-private (inner-save-count-to-wrbpod (count uint))
    (let (
        (wrbpod-session (unwrap! (wrbpod-open (wrbpod-default))
            (err (errmsg "save-count-to-wrbpod: Failed to open default wrbpod"))))
    )
    (match (wrbpod-fetch-slot wrbpod-session u0)
        slot true
        err-txt false)
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
        err-res (err { code: u1, message: (display-errmsg err-res) }))))

(define-public (main (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (let (
        (events (var-get event-count))
        (err-ct (var-get err-countdown))
        (wrbpod-count (match (get-count-from-wrbpod)
            value value
            error u111111111))
    )
    (try! (setup-wrbpod))

    (try! (wrb-viewport-clear VIEWPORT_STATUS))

    ;; update error
    (if (is-eq err-ct u0)
        (try! (wrb-viewport-clear VIEWPORT_ERROR))
        (update-errmsg-counter err-ct))

    (try! (wrb-txt VIEWPORT_STATUS u0 u0 BLACK WHITE (concat u"Ran event loop " (concat (int-to-utf8 events) u" time(s)" ))))
    (try! (wrb-txt VIEWPORT_STATUS u1 u0 BLACK WHITE (concat u"Count from wrbpod: " (int-to-utf8 wrbpod-count))))

    (var-set event-count (+ u1 events))

    (try! (save-count-to-wrbpod events))
 
    (ok true)))

(wrb-event-loop "main")
(wrb-event-subscribe WRB_EVENT_CLOSE)
(wrb-event-subscribe WRB_EVENT_TIMER)
(wrb-event-loop-time u1000)

