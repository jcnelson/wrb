(wrb-root u120 u120)

(define-constant VIEWPORT_STATUS u0)
(define-constant VIEWPORT_WIDGETS u1)

(define-constant BLACK u0)
(define-constant WHITE (buff-to-uint-le 0xffffff))

(wrb-viewport VIEWPORT_STATUS u0 u0 u120 u60)
(wrb-viewport VIEWPORT_WIDGETS u0 u60 u120 u60)

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
    (wrb-textarea VIEWPORT_WIDGETS u15 u0 u20 u60 (* u2 u20 u260) u"Initial text"))

(define-data-var event-count uint u0)

(define-public (main (element-type uint) (element-id uint) (event-type uint) (event-payload (buff 1024)))
    (let (
        (events (var-get event-count))
    )
    (try! (wrb-viewport-clear VIEWPORT_STATUS))
    (try! (wrb-txt VIEWPORT_STATUS u0 u0 BLACK WHITE (concat u"Ran event loop " (concat (int-to-utf8 events) u" time(s)" ))))
    (var-set event-count (+ u1 events))
    (ok true)))

(wrb-event-loop "main")
(wrb-event-subscribe WRB_EVENT_CLOSE)
(wrb-event-subscribe WRB_EVENT_TIMER)
(wrb-event-loop-time u1000)

