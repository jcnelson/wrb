;; Wrapper around .wrb functions.
;; This gets linked directly into the wrb application.

;; event types
(define-constant WRB_EVENT_CLOSE u0)
(define-constant WRB_EVENT_TIMER u1)
(define-constant WRB_EVENT_RESIZE u2)
(define-constant WRB_EVENT_OPEN u3)
(define-constant WRB_EVENT_UI u4)

;; get the app name and version
(define-private (wrb-get-app-name)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb get-app-name))

;; read-only call to the node
(define-private (wrb-call-readonly? (contract principal) (function-name (string-ascii 128)) (function-args-list (buff 102400)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb call-readonly contract function-name function-args-list))

;; buff to utf8 string
(define-private (wrb-buff-to-string-utf8? (arg (buff 102400)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb buff-to-string-utf8 arg))

;; ascii string to utf8 string
(define-private (wrb-string-ascii-to-string-utf8? (arg (string-ascii 25600)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb string-ascii-to-string-utf8 arg))

;; store a large utf8 string behind a handle
(define-private (wrb-store-large-string-utf8 (handle uint) (txt (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb store-large-string-utf8 handle txt)))

;; load a large utf8 string
;; returns (optional (string-utf8 12800))
(define-private (wrb-load-large-string-utf8 (handle uint))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb load-large-string-utf8 handle)))

;; used internally
(define-read-only (internal-cache-bypass-load-large-string-utf8 (handle uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb internal-cache-bypass-load-large-string-utf8 handle))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Root and Viewports ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Define the size of the root viewport in rows and columns
(define-private (wrb-root (cols uint) (rows uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-root cols rows))

;; Get the root description
(define-read-only (wrb-get-root)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb get-wrb-root))

;; Declare a viewport
(define-private (wrb-viewport (id uint) (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport id start-row start-col num-rows num-cols))

;; Declare a child viewport of an existing parent viewport
(define-private (wrb-viewport-child (id uint) (parent-id uint) (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-child-viewport id parent-id start-row start-col num-rows num-cols))

;; Set a viewport's dimensions
(define-public (wrb-viewport-set-dims (id uint) (rows uint) (cols uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb set-viewport-dims id rows cols))

;; Get the viewports. Really meant for internal consumption to iterate through viewports in the renderer.
(define-read-only (wrb-get-viewports (cursor (optional uint)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb get-viewports cursor))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Viewport text ;;;;;;;;;;;;;;;;;;;;;yy;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Add static text to a viewport, with a specific color
(define-private (wrb-static-txt-immediate (id uint) (row uint) (col uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-static-text-immediate id row col bg-color fg-color text)))

;; Print text to a viewport, no newline, with given foreground/background colors and a specific cursor.
(define-private (wrb-static-print-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-static-print-immediate id cursor bg-color fg-color text)))

;; Print text to a viewport with newline, with given foreground/background colors and a specific cursor
(define-private (wrb-static-println-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-static-println-immediate id cursor bg-color fg-color text)))

;; Add dynamic text to a viewport, with a specific color
;; Must be called on each frame to be persistent
(define-private (wrb-txt-immediate (id uint) (row uint) (col uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-text-immediate id row col bg-color fg-color text)))

;; Print text to a viewport, no newline, with given foreground/background colors and a specific cursor.
;; Must be called on each frame to be persistent
(define-private (wrb-print-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-print-immediate id cursor bg-color fg-color text)))

;; Print text to a viewport with newline, with given foreground/background colors and a specific cursor
;; Must be called on each frame to be persistent
(define-private (wrb-println-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-println-immediate id cursor bg-color fg-color text)))

;; Set static text colors for a viewport
(define-private (wrb-set-static-txt-colors (viewport-id uint) (fg uint) (bg uint))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-set-static-txt-colors viewport-id fg bg)))

;; Set dynamic text colors for a viewport
(define-private (wrb-set-txt-colors (viewport-id uint) (fg uint) (bg uint))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-set-txt-colors viewport-id fg bg)))

;; Add static text to a viewport, using its default color
(define-private (wrb-static-txt (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-static-text id row col text)))

;; Print text to a viewport, no newline, with a specific cursor.
(define-private (wrb-static-print (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-static-print id cursor text)))

;; Print text to a viewport with newline, with a specific cursor
(define-private (wrb-static-println (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-static-println id cursor text)))

;; Add dynamic text to a viewport.
;; Must be called on each frame to be persistent
(define-private (wrb-txt (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-text id row col text)))

;; Print text to a viewport, no newline, with ga specific cursor.
;; Must be called on each frame to be persistent
(define-private (wrb-print (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-print id cursor text)))

;; Print text to a viewport with newline, with a specific cursor
;; Must be called on each frame to be persistent
(define-private (wrb-println (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-println id cursor text)))

;; Clear the viewport of text
(define-private (wrb-viewport-clear (id uint))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-clear id)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Viewport UI elements  ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Add a button to the viewport.
;; Returns its UI element ID.
(define-private (wrb-button (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-add-button id row col text)))

;; Add a checkbox list to the viewport.
;; Returns its UI element ID.
(define-private (wrb-checkbox (id uint) (row uint) (col uint) (options (list 256 { text: (string-utf8 200), selected: bool })))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-add-checkbox id row col options)))

;; Add a textline to the viewport.
;; Returns its UI element ID.
(define-private (wrb-textline (id uint) (row uint) (col uint) (max-len uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-add-textline id row col max-len text)))

;; Add a textarea to the viewport.
;; Returns its UI element ID.
(define-private (wrb-textarea (id uint) (row uint) (col uint) (num-rows uint) (num-cols uint) (max-len uint) (text (string-utf8 12800)))
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-add-textarea id row col num-rows num-cols max-len text)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Viewport UI queries  ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Get the number of UI elements (used internally)
(define-read-only (wrb-ui-len)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-len))

;; Get a UI element descriptor at a particular index (used internally)
(define-read-only (wrb-ui-element-descriptor (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-element-descriptor index))

;; Get a text UI element at a particular index (used internally)
(define-read-only (wrb-ui-get-text-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-text-element index))

;; Get a print UI element at a particular index (used internally)
(define-read-only (wrb-ui-get-print-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-print-element index))

;; Get a button UI element at a particular index (used internally)
(define-read-only (wrb-ui-get-button-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-button-element index))

;; Get a checkbox UI element at a particular index (used internally)
(define-read-only (wrb-ui-get-checkbox-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-checkbox-element index))

;; Get a textline UI element at a particular index (used internally)
(define-read-only (wrb-ui-get-textline-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-textline-element index))

;; Get a textarea UI element at a particular index (used internally)
(define-read-only (wrb-ui-get-textarea-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-textarea-element index))

;; Get all dynamic text statements
(define-read-only (wrb-dynamic-ui-get-text-elements)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-dynamic-ui-get-text-elements))

;; Get all dynamic print statements
(define-read-only (wrb-dynamic-ui-get-print-elements)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-dynamic-ui-get-print-elements))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Wrbpods  ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Get the address of the user's wrbpod
(define-private (wrbpod-default)
    (unwrap-panic (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-default)))

;; Open a connection to a wrbpod
(define-private (wrbpod-open (superblock { contract: principal, slot: uint }))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-open superblock))

;; Get the number of slots that the app owns.
;; Returns (response uint { code: uint, message: (string-ascii 512) })
(define-public (wrbpod-get-num-slots (session-id uint) (app-name { name: (buff 48), namespace: (buff 20) }))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-get-num-slots session-id app-name))

;; Allocate slots in a wrbpod that the user owns
;; Returns (response bool { code: uint, message: (string-ascii 512) }), where
;; (ok true) indicates successful allocation and
;; (ok false) indicates a failure to allocate.
(define-private (wrbpod-alloc-slots (session-id uint) (num-slots uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-alloc-slots session-id num-slots))

;; Download a local copy of a wrbpod slot for editing.
;; Slots are 0-indexed from 0 inclusive to the number of slots obtained
;; by (wrbpod-get-num-slots) exclusive.
;; The slot cannot be directly edited; instead, the app uses
;; the (wrbpod-get-slice) and (wrbpod-put-slice) functions to 
;; load and store indexed bytestrings within the slot, respectively.
;; Returns (response { version: uint, signer: (optional principal) } { code: uint, message: (string-ascii 512)})
(define-private (wrbpod-fetch-slot (session-id uint) (slot-id uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-fetch-slot session-id slot-id))

;; Get a slice of a locally-fetched slot.
(define-private (wrbpod-get-slice (session-id uint) (slot-id uint) (slice-id uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-get-slice session-id slot-id slice-id))

;; Put a slice into a locally-fetched slot, but don't upload it yet
(define-private (wrbpod-put-slice (session-id uint) (slot-id uint) (slice-id uint) (data-slice (buff 786000)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-put-slice session-id slot-id slice-id data-slice))

;; Upload a slot
(define-private (wrbpod-sync-slot (session-id uint) (slot-id uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-sync-slot session-id slot-id))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Event loop ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Set the wrb event loop
(define-public (wrb-event-loop (func-name (string-ascii 512)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-event-loop func-name))

;; Subscribe to an event type
(define-public (wrb-event-subscribe (event-type uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-event-subscribe event-type))

;; Get the name of the event loop function (used internally)
(define-read-only (wrb-get-event-loop-name)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-get-event-loop-name))

;; Get the number of event subscriptions (used internally)
(define-read-only (wrb-get-num-event-subscriptions)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-get-num-event-subscriptions))

;; Get an event subscription
(define-read-only (wrb-get-event-subscription (idx uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-get-event-subscription idx))

;; Set event loop delay
(define-public (wrb-event-loop-time (delay-ms uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-event-loop-time delay-ms))

;; Get the event loop delay
(define-read-only (wrb-get-event-loop-time)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-get-event-loop-time))


