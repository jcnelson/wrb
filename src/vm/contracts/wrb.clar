;; UI element type IDs
(define-constant WRB_UI_TYPE_TEXT u4)
(define-constant WRB_UI_TYPE_PRINT u5)
(define-constant WRB_UI_TYPE_BUTTON u6)
(define-constant WRB_UI_TYPE_CHECKBOX u7)
(define-constant WRB_UI_TYPE_TEXTLINE u8)
(define-constant WRB_UI_TYPE_TEXTAREA u9)

;; Special event types, beyond UI events
(define-constant WRB_EVENT_CLOSE u0)
(define-constant WRB_EVENT_TIMER u1)
(define-constant WRB_EVENT_RESIZE u2)
(define-constant WRB_EVENT_OPEN u3)

(define-constant UPPER_u128 u170141183460469231731687303715884105728)

;; Call a read-only function 
(define-public (call-readonly (contract principal) (function-name (string-ascii 128)) (function-args-list (buff 102400)))
   (begin
       (unwrap-panic (contract-call? .wrb-ll call-readonly contract function-name function-args-list))
       (contract-call? .wrb-ll get-last-call-readonly)))

;; Get an attachment
(define-public (get-attachment (attachment-hash (buff 20)))
   (begin
       (unwrap-panic (contract-call? .wrb-ll get-attachment attachment-hash))
       (contract-call? .wrb-ll get-last-attachment)))

;; Tries to converts a buff to a string-utf8
(define-public (buff-to-string-utf8 (arg (buff 102400)))
   (begin
       (unwrap-panic (contract-call? .wrb-ll buff-to-string-utf8 arg))
       (contract-call? .wrb-ll get-last-wrb-buff-to-string-utf8)))

(define-data-var wrb-root-size { cols: uint, rows: uint } { cols: u0, rows: u0 })

;; Set the root pane's dimensions
(define-public (wrb-root (rows uint) (cols uint))
   (ok (var-set wrb-root-size { cols: cols, rows: rows })))

;; Get the root pane's dimensions
(define-read-only (get-wrb-root)
   (var-get wrb-root-size))

(define-map viewports
    ;; viewport ID in use
    uint
    ;; viewport data
    {
       start-col: uint,
       start-row: uint,
       num-cols: uint,
       num-rows: uint,
       visible: bool,
       parent: (optional uint),
       last: (optional uint)
    })

(define-data-var last-viewport-id (optional uint) none)

(define-private (ascii-512 (str (string-ascii 512)))
    (unwrap-panic (as-max-len? str u512)))

(define-private (check-dims (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
    (begin
       (asserts! (< start-col u65536) (err (ascii-512 "start-col too big")))
       (asserts! (< start-row u65536) (err (ascii-512 "start-row too big")))
       (asserts! (< (+ start-col num-cols) u65536) (err (ascii-512 "num-cols too big")))
       (asserts! (< (+ start-row num-rows) u65536) (err (ascii-512 "num-rows too big")))
       (ok true)))
     
;; Add a root-level viewport 
(define-public (wrb-viewport (id uint) (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
   (begin
        (try! (check-dims start-row start-col num-rows num-cols))
        (asserts! (is-none (map-get? viewports id)) (err (ascii-512 "viewport already exists")))
        (map-set viewports
            id
            {
                start-col: start-col,
                start-row: start-row,
                num-cols: num-cols,
                num-rows: num-rows,
                visible: true,
                parent: none,
                last: (var-get last-viewport-id)
            })
        (var-set last-viewport-id (some id))
        (ok true)))

;; Add a viewport within an existing viewport
(define-public (wrb-child-viewport (id uint) (parent-id uint) (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
   (begin
        (try! (check-dims start-row start-col num-rows num-cols))
        (asserts! (is-none (map-get? viewports id)) (err (ascii-512 "viewport already exists")))
        (asserts! (is-some (map-get? viewports parent-id)) (err (ascii-512 "parent viewport does not exist")))
        (map-set viewports
            id
            {
                start-col: start-col,
                start-row: start-row,
                num-cols: num-cols,
                num-rows: num-rows,
                visible: true,
                parent: (some parent-id),
                last: (var-get last-viewport-id)
            })
        (var-set last-viewport-id (some id))
        (ok true)))

(define-read-only (get-viewports-iter (ignored bool) (state { cursor: (optional uint), viewports: (list 1024 { id: uint, start-col: uint, start-row: uint, num-cols: uint, num-rows: uint, visible: bool, parent: (optional uint), last: (optional uint) })}))
    (match (get cursor state)
        cursor (let (
            (next-viewport (map-get? viewports cursor))
            (viewport-list (get viewports state)))
            (match next-viewport
                viewport {
                    cursor: (get last viewport),
                    viewports: (default-to viewport-list (as-max-len? (append viewport-list (merge { id: cursor } viewport)) u1024))
                }
                state
            ))
        state))
            
(define-read-only (get-viewports (cursor (optional uint)))
    (get viewports (fold get-viewports-iter (list true true true true true true true true true true true true true true true true true true true true)
        { cursor: (if (is-none cursor) (var-get last-viewport-id) cursor), viewports: (list ) })))

(define-map wrb-ui-list
   ;; index
   uint
   ;; element
   {
       viewport: uint,
       type: uint
   })

(define-data-var wrb-ui-list-len uint u0)

(define-map wrb-dynamic-ui-list
   ;; indexed by viewport and insert order
   {
       viewport: uint,
       index: uint
   }
   ;; index into dynamic-viewport-text-list or dynamic-viewport-print-list
   {
       ;; this is the key to the above maps
       ui-index: uint,
       ;; this tells us which map to use
       type: uint
   })

(define-map wrb-dynamic-ui-list-start
    ;; viewport ID
    uint
    ;; smallest index into `wrb-dynamic-ui-list` for this viewport
    uint)

(define-map wrb-dynamic-ui-list-end
    ;; viewport id
    uint
    ;; highest index into `wrb-dynamic-ui-list` for this viewport
    uint)

;; generates keys into the `dynamic-viewport-*-list` maps
(define-data-var wrb-dynamic-ui-list-len uint u0)

(define-map viewport-text-list
   ;; index
   uint
   ;; payload
   {
       element-id: uint,
       text: (string-utf8 12800),
       col: uint,
       row: uint,
       bg-color: uint,
       fg-color: uint
   })

(define-map dynamic-viewport-text-list
   ;; index
   uint
   ;; payload
   {
       element-id: uint,
       text: (string-utf8 12800),
       col: uint,
       row: uint,
       bg-color: uint,
       fg-color: uint
   })

(define-map viewport-print-list
   ;; index
   uint
   ;; payload
   {
       element-id: uint,
       text: (string-utf8 12800),
       cursor: (optional { col: uint, row: uint }),
       bg-color: uint,
       fg-color: uint,
       newline: bool
   })

(define-map dynamic-viewport-print-list
   ;; index
   uint
   ;; payload
   {
       element-id: uint,
       text: (string-utf8 12800),
       cursor: (optional { col: uint, row: uint }),
       bg-color: uint,
       fg-color: uint,
       newline: bool
   })

(define-map viewport-button-list
    ;; index
    uint
    ;; payload
    {
       element-id: uint,
       text: (string-utf8 12800),
       col: uint,
       row: uint,
       bg-color: uint,
       fg-color: uint,
       focused-bg-color: uint,
       focused-fg-color: uint,
    })

(define-map viewport-checkbox-list
    ;; index
    uint
    ;; payload
    {
       element-id: uint,
       col: uint,
       row: uint,
       bg-color: uint,
       fg-color: uint,
       focused-bg-color: uint,
       focused-fg-color: uint,
       selector-color: uint,
       options: (list 256 { text: (string-utf8 200), selected: bool })
    })

(define-map viewport-textline-list
    ;; index
    uint
    ;; payload
    {
       element-id: uint,
       col: uint,
       row: uint,
       bg-color: uint,
       fg-color: uint,
       focused-bg-color: uint,
       focused-fg-color: uint,
       max-len: uint,
       text: (string-utf8 12800)
    })

(define-map viewport-textarea-list
    ;; index
    uint
    ;; payload
    {
       element-id: uint,
       col: uint,
       row: uint,
       num-rows: uint,
       num-cols: uint,
       bg-color: uint,
       fg-color: uint,
       focused-bg-color: uint,
       focused-fg-color: uint,
       max-len: uint,
       text: (string-utf8 12800)
    })

;; Add static raw text to a viewport
(define-public (wrb-viewport-static-text (id uint) (row uint) (col uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; add text element
   (map-set viewport-text-list
       ui-list-len
       { element-id: ui-list-len, row: row, col: col, bg-color: bg-color, fg-color: fg-color, text: text })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_TEXT })

   ;; next UI element
   (var-set wrb-ui-list-len (+ u1 ui-list-len))
   (if true
       (ok true)
       (err (ascii-512 "infallible")))
))

;; Print static text to a viewport, with wordwrap.
(define-public (wrb-viewport-static-print (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; add text element
   (map-set viewport-print-list
       ui-list-len
       { element-id: ui-list-len, cursor: cursor, bg-color: bg-color, fg-color: fg-color, text: text, newline: false })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_PRINT })

   ;; next UI element
   (var-set wrb-ui-list-len (+ u1 ui-list-len))
   (if true
       (ok true)
       (err (ascii-512 "infallible")))
))

;; Print static text to a viewport, with wordwrap and newline
(define-public (wrb-viewport-static-println (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; add text element
   (map-set viewport-print-list
       ui-list-len
       { element-id: ui-list-len, cursor: cursor, bg-color: bg-color, fg-color: fg-color, text: text, newline: true })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_PRINT })

   ;; next UI element
   (var-set wrb-ui-list-len (+ u1 ui-list-len))
   (if true
       (ok true)
       (err (ascii-512 "infallible")))
))

;; Print dynamic text to a viewport
(define-public (wrb-viewport-text (id uint) (row uint) (col uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (dynamic-ui-list-len (var-get wrb-dynamic-ui-list-len))
       (viewport-dynamic-ui-list-end (default-to u0 (map-get? wrb-dynamic-ui-list-end id)))
   )
   ;; add text element
   (map-set dynamic-viewport-text-list
       dynamic-ui-list-len
       { element-id: (+ dynamic-ui-list-len UPPER_u128), row: row, col: col, bg-color: bg-color, fg-color: fg-color, text: text })

   ;; register UI element
   (map-set wrb-dynamic-ui-list
       { viewport: id, index: viewport-dynamic-ui-list-end }
       { ui-index: dynamic-ui-list-len, type: WRB_UI_TYPE_TEXT })

   ;; next UI element
   (var-set wrb-dynamic-ui-list-len (+ u1 dynamic-ui-list-len))
   (map-set wrb-dynamic-ui-list-end id (+ u1 viewport-dynamic-ui-list-end))
   (if true
       (ok true)
       (err (ascii-512 "infallible")))))

;; Print dynamic text to a viewport, with wordwrap.
(define-public (wrb-viewport-print (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (dynamic-ui-list-len (var-get wrb-dynamic-ui-list-len))
       (viewport-dynamic-ui-list-end (default-to u0 (map-get? wrb-dynamic-ui-list-end id)))
   )
   ;; add text element
   (map-set dynamic-viewport-print-list
       dynamic-ui-list-len
       { element-id: (+ dynamic-ui-list-len UPPER_u128), cursor: cursor, bg-color: bg-color, fg-color: fg-color, text: text, newline: false })

   ;; register UI element
   (map-set wrb-dynamic-ui-list
       { viewport: id, index: viewport-dynamic-ui-list-end }
       { ui-index: dynamic-ui-list-len, type: WRB_UI_TYPE_PRINT })

   ;; next UI element
   (var-set wrb-dynamic-ui-list-len (+ u1 dynamic-ui-list-len))
   (map-set wrb-dynamic-ui-list-end id (+ u1 viewport-dynamic-ui-list-end))
   (if true
       (ok true)
       (err (ascii-512 "infallible")))))

;; Print dynamic text to a viewport, with wordwrap and newline.
(define-public (wrb-viewport-println (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (dynamic-ui-list-len (var-get wrb-dynamic-ui-list-len))
       (viewport-dynamic-ui-list-end (default-to u0 (map-get? wrb-dynamic-ui-list-end id)))
   )
   ;; add text element
   (map-set dynamic-viewport-print-list
       dynamic-ui-list-len
       { element-id: (+ dynamic-ui-list-len UPPER_u128), cursor: cursor, bg-color: bg-color, fg-color: fg-color, text: text, newline: true })

   ;; register UI element
   (map-set wrb-dynamic-ui-list
       { viewport: id, index: viewport-dynamic-ui-list-end }
       { ui-index: dynamic-ui-list-len, type: WRB_UI_TYPE_PRINT })

   ;; next UI element
   (var-set wrb-dynamic-ui-list-len (+ u1 dynamic-ui-list-len))
   (map-set wrb-dynamic-ui-list-end id (+ u1 viewport-dynamic-ui-list-end))
   (if true
       (ok true)
       (err (ascii-512 "infallible")))))

;; Clear a viewport of text
(define-public (wrb-viewport-clear (id uint))
   (let (
       (list-len (default-to u0 (map-get? wrb-dynamic-ui-list-end id)))
   )
   (map-set wrb-dynamic-ui-list-start id list-len)
   (if true
       (ok true)
       (err (ascii-512 "infallible")))))

;; Default button style
(define-data-var wrb-default-button-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-button-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })

;; Add a button to a viewport
;; Returns the button ID
(define-public (wrb-viewport-add-button (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (button-color (var-get wrb-default-button-colors))
        (focused-button-color (var-get wrb-default-focused-button-colors))
    )
    ;; add button element
    (map-set viewport-button-list
        ui-list-len
        {
            element-id: ui-list-len,
            text: text,
            col: col,
            row: row,
            fg-color: (get fg button-color),
            bg-color: (get bg button-color),
            focused-fg-color: (get fg focused-button-color),
            focused-bg-color: (get bg focused-button-color)
        })

    ;; register UI element
    (map-set wrb-ui-list
        ui-list-len
        { viewport: id, type: WRB_UI_TYPE_BUTTON })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok ui-list-len)
))

;; Default checkbox style
(define-data-var wrb-default-checkbox-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-checkbox-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })
(define-data-var wrb-default-checkbox-selector-color uint u16711680)

;; Add a checkbox group to a viewport
;; Returns the checkbox ID
(define-public (wrb-viewport-add-checkbox (id uint) (row uint) (col uint) (options (list 256 { text: (string-utf8 200), selected: bool })))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (checkbox-color (var-get wrb-default-checkbox-colors))
        (focused-checkbox-color (var-get wrb-default-focused-checkbox-colors))
        (selector-color (var-get wrb-default-checkbox-selector-color))
    )
    ;; add checkbox element
    (map-set viewport-checkbox-list
        ui-list-len
        {
            element-id: ui-list-len,
            col: col,
            row: row,
            fg-color: (get fg checkbox-color),
            bg-color: (get bg checkbox-color),
            focused-fg-color: (get fg focused-checkbox-color),
            focused-bg-color: (get bg focused-checkbox-color),
            selector-color: selector-color,
            options: options
        })

    ;; register UI element
    (map-set wrb-ui-list
        ui-list-len
        { viewport: id, type: WRB_UI_TYPE_CHECKBOX })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok ui-list-len)
))

;; Default text line style
(define-data-var wrb-default-textline-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-textline-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })

;; Add a textline to a viewport
;; Returns the textline ID
(define-public (wrb-viewport-add-textline (id uint) (row uint) (col uint) (max-len uint) (text (string-utf8 12800)))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (textline-color (var-get wrb-default-textline-colors))
        (focused-textline-color (var-get wrb-default-focused-textline-colors))
    )
    ;; add textline element
    (map-set viewport-textline-list
        ui-list-len
        {
            element-id: ui-list-len,
            col: col,
            row: row,
            fg-color: (get fg textline-color),
            bg-color: (get bg textline-color),
            focused-fg-color: (get fg focused-textline-color),
            focused-bg-color: (get bg focused-textline-color),
            max-len: max-len,
            text: text
        })

    ;; register UI element
    (map-set wrb-ui-list
        ui-list-len
        { viewport: id, type: WRB_UI_TYPE_TEXTLINE })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok ui-list-len)
)) 

;; Default text area style
(define-data-var wrb-default-textarea-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-textarea-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })

;; Add a textarea to a viewport
;; Returns the textarea ID
(define-public (wrb-viewport-add-textarea (id uint) (row uint) (col uint) (num-rows uint) (num-cols uint) (max-len uint) (text (string-utf8 12800)))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (textarea-color (var-get wrb-default-textarea-colors))
        (focused-textarea-color (var-get wrb-default-focused-textarea-colors))
    )
    ;; add textarea element
    (map-set viewport-textarea-list
        ui-list-len
        {
            element-id: ui-list-len,
            col: col,
            row: row,
            num-rows: num-rows,
            num-cols: num-cols,
            fg-color: (get fg textarea-color),
            bg-color: (get bg textarea-color),
            focused-fg-color: (get fg focused-textarea-color),
            focused-bg-color: (get bg focused-textarea-color),
            max-len: max-len,
            text: text
        })

    ;; register UI element
    (map-set wrb-ui-list
        ui-list-len
        { viewport: id, type: WRB_UI_TYPE_TEXTAREA })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok ui-list-len)
)) 

;; Get the number of UI elements
(define-read-only (wrb-ui-len)
   (var-get wrb-ui-list-len))

;; Get a UI element descriptor
(define-read-only (wrb-ui-element-descriptor (index uint))
   (map-get? wrb-ui-list index))

;; Get a text element
(define-read-only (wrb-ui-get-text-element (index uint))
   (map-get? viewport-text-list index))

;; Get a print element
(define-read-only (wrb-ui-get-print-element (index uint))
   (map-get? viewport-print-list index))

;; Get a button element
(define-read-only (wrb-ui-get-button-element (index uint))
   (map-get? viewport-button-list index))

;; Get a checkbox element
(define-read-only (wrb-ui-get-checkbox-element (index uint))
   (map-get? viewport-checkbox-list index))

;; Get a textline element
(define-read-only (wrb-ui-get-textline-element (index uint))
   (map-get? viewport-textline-list index))

;; Get a textarea element
(define-read-only (wrb-ui-get-textarea-element (index uint))
   (map-get? viewport-textarea-list index))

;; Get the minimum dynamic text index for a viewport
(define-read-only (wrb-dynamic-ui-index-start (id uint))
    (default-to u0 (map-get? wrb-dynamic-ui-list-start id)))

;; Get the minimum dynamic text index for a viewport
(define-read-only (wrb-dynamic-ui-index-end (id uint))
    (default-to u0 (map-get? wrb-dynamic-ui-list-end id)))

;; Get a dynamic UI pointer
(define-read-only (wrb-dynamic-ui-pointer (viewport uint) (index uint))
    (map-get? wrb-dynamic-ui-list { viewport: viewport, index: index }))

;; Get a dynamic text statement 
(define-read-only (wrb-dynamic-ui-get-text-element (ui-index uint))
    (map-get? dynamic-viewport-text-list ui-index))

;; Get a dynamic print statement 
(define-read-only (wrb-dynamic-ui-get-print-element (ui-index uint))
    (map-get? dynamic-viewport-print-list ui-index))

;; Open a wrbpod. Creates a session for it and returns the session ID (as a uint)
(define-public (wrbpod-open (stackerdb-contract principal))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-open stackerdb-contract))
        (contract-call? .wrb-ll get-last-wrbpod-open-result)))

;; How many slots are allocated to this app in the wrbpod?
(define-public (wrbpod-get-num-slots (session-id uint) (app-name { name: (buff 48), namespace: (buff 20) }))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-get-num-slots session-id app-name))
        (contract-call? .wrb-ll get-last-wrbpod-get-num-slots)))

;; Ask for more slots to be allocated to this running wrbsite for an open wrbpod.
;; This only works if the wrbsite has write access to the wrbpod superblock.
(define-public (wrbpod-alloc-slots (session-id uint) (num-slots uint))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-alloc-slots session-id num-slots))
        (contract-call? .wrb-ll get-last-wrbpod-alloc-slots-result)))

;; Fetch a slot within a wrbpod. Downloads the latest copy and caches the signer and version information.
;; Returns the signer and version information.
(define-public (wrbpod-fetch-slot (session-id uint) (slot-id uint))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-fetch-slot session-id slot-id))
        (contract-call? .wrb-ll get-wrbpod-fetch-slot-result session-id slot-id)))

;; Get a slice within a fetched slot.
(define-public (wrbpod-get-slice (session-id uint) (slot-id uint) (slice-id uint))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-get-slice session-id slot-id slice-id))
        (contract-call? .wrb-ll get-wrbpod-get-slice-result session-id slot-id slice-id)))

;; Put a slice into an fetched slot.
;; The slice won't be persisted until a subsequent call to wrbpod-sync-slot.
(define-public (wrbpod-put-slice (session-id uint) (slot-id uint) (slice-id uint) (data-slice (buff 786000)))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-put-slice session-id slot-id slice-id data-slice))
        (contract-call? .wrb-ll get-wrbpod-put-slice-result session-id slot-id slice-id)))

;; Synchronize a dirty slot
(define-public (wrbpod-sync-slot (session-id uint) (slot-id uint))
    (begin
        (try! (contract-call? .wrb-ll wrbpod-sync-slot session-id slot-id))
        (contract-call? .wrb-ll get-last-wrbpod-sync-slot-result session-id slot-id)))

;; Register the main event loop
(define-data-var wrb-event-loop-name (optional (string-ascii 512)) none)
(define-public (wrb-event-loop (function-name (string-ascii 512)))
    (ok (var-set wrb-event-loop-name (some function-name))))
(define-read-only (wrb-get-event-loop-name)
    (var-get wrb-event-loop-name))

;; Subscribe to a particular category of events
(define-map wrb-event-subscriptions
    ;; index
    uint
    ;; event ID
    uint)

(define-data-var wrb-last-event-subscription uint u0)
(define-public (wrb-event-subscribe (event-id uint))
    (let (
        (last-event-index (var-get wrb-last-event-subscription))
    )
    (map-insert wrb-event-subscriptions last-event-index event-id)
    (var-set wrb-last-event-subscription (+ u1 last-event-index) )
    (ok true)))

(define-read-only (wrb-get-num-event-subscriptions)
    (var-get wrb-last-event-subscription))
(define-read-only (wrb-get-event-subscription (idx uint))
    (map-get? wrb-event-subscriptions idx))

;; Event loop timing config
;; Delay is in ms
(define-data-var wrb-event-loop-delay uint u33)
(define-public (wrb-event-loop-time (delay uint))
    (ok (var-set wrb-event-loop-delay delay)))
(define-read-only (wrb-get-event-loop-time)
    (var-get wrb-event-loop-delay))

(begin
   (print "wrb is not the web"))
