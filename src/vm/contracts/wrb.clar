;; UI toolkit and wrappers around wrb-ll

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
(define-constant WRB_EVENT_UI u4)

;; Error types (copied from wrb-ll)
(define-constant WRB_ERR_INFALLIBLE u0)
(define-constant WRB_ERR_INVALID u1)
(define-constant WRB_ERR_EXISTS u2)
(define-constant WRB_ERR_NOT_FOUND u3)

(define-constant WRB_ERR_WRBPOD_NOT_OPEN u1000)
(define-constant WRB_ERR_WRBPOD_NO_SLOT u1001)
(define-constant WRB_ERR_WRBPOD_NO_SLICE u1002)
(define-constant WRB_ERR_WRBPOD_OPEN_FAILURE u1003)
(define-constant WRB_ERR_WRBPOD_SLOT_ALLOC_FAILURE u1004)
(define-constant WRB_ERR_WRBPOD_FETCH_SLOT_FAILURE u1005)
(define-constant WRB_ERR_WRBPOD_PUT_SLICE_FAILURE u1006)
(define-constant WRB_ERR_WRBPOD_SYNC_SLOT_FAILURE u1007)

(define-constant WRB_ERR_READONLY_FAILURE u2000)

(define-constant WRB_ERR_BUFF_TO_UTF8_FAILURE u3000)

(define-constant WRB_ERR_ASCII_TO_UTF8_FAILURE u4000)

;; constants
(define-constant UPPER_u128 u170141183460469231731687303715884105728)

;; Error constructor
(define-private (err-ascii-512 (code uint) (str (string-ascii 512)))
    { code: code, message: (unwrap-panic (as-max-len? str u512)) })

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb App Config ;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Get the app name
(define-read-only (wrb-get-app-name)
    (contract-call? .wrb-ll wrb-ll-get-app-name))

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb Node RPC ;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Call a read-only function 
(define-private (wrb-call-readonly? (contract principal) (function-name (string-ascii 128)) (function-args-list (buff 102400)))
   (begin
       (unwrap-panic (contract-call? .wrb-ll wrb-ll-call-readonly contract function-name function-args-list))
       (contract-call? .wrb-ll wrb-ll-get-last-call-readonly)))

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb String Utils ;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Tries to converts a buff to a string-utf8
(define-private (wrb-buff-to-string-utf8? (arg (buff 102400)))
   (begin
       (unwrap-panic (contract-call? .wrb-ll wrb-ll-buff-to-string-utf8 arg))
       (contract-call? .wrb-ll wrb-ll-get-last-wrb-buff-to-string-utf8)))

;; Tries to converts a string-ascii to a string-utf8
(define-private (wrb-string-ascii-to-string-utf8? (arg (string-ascii 25600)))
   (begin
       (unwrap-panic (contract-call? .wrb-ll wrb-ll-string-ascii-to-string-utf8 arg))
       (contract-call? .wrb-ll wrb-ll-get-last-wrb-string-ascii-to-string-utf8)))

;; Store a large string
(define-private (wrb-store-large-string-utf8 (handle uint) (value (string-utf8 12800)))
    (unwrap-panic (contract-call? .wrb-ll wrb-ll-store-large-string-utf8 handle value)))

;; Load a previously-stored large string
(define-private (wrb-load-large-string-utf8 (handle uint))
    (begin
        (unwrap-panic (contract-call? .wrb-ll wrb-ll-load-large-string-utf8 handle))
        (contract-call? .wrb-ll wrb-ll-get-last-load-large-string-utf8)))

;; Load a previously-stored large string, but bypassing the wrb runtime cache
;; (used internally)
(define-read-only (wrb-internal-cache-bypass-load-large-string-utf8 (handle uint))
    (contract-call? .wrb-ll wrb-ll-cache-bypass-load-large-string-utf8 handle))

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb Root ;;;;;;;;;;;;;;;;;;;;;;;;;;

(define-data-var wrb-root-size { cols: uint, rows: uint } { cols: u0, rows: u0 })

;; Set the root pane's dimensions
(define-private (wrb-root (rows uint) (cols uint))
   (var-set wrb-root-size { cols: cols, rows: rows }))

;; Get the root pane's dimensions
(define-read-only (wrb-get-root)
   (var-get wrb-root-size))

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb Viewports ;;;;;;;;;;;;;;;;;;;;;;;;;;

(define-map wrb-viewports
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

(define-data-var wrb-last-viewport-id (optional uint) none)

(define-private (wrb-check-dims (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
    (begin
       (asserts! (< start-col u65536) (err (err-ascii-512 WRB_ERR_INVALID "start-col too big")))
       (asserts! (< start-row u65536) (err (err-ascii-512 WRB_ERR_INVALID "start-row too big")))
       (asserts! (< (+ start-col num-cols) u65536) (err (err-ascii-512 WRB_ERR_INVALID "num-cols too big")))
       (asserts! (< (+ start-row num-rows) u65536) (err (err-ascii-512 WRB_ERR_INVALID "num-rows too big")))
       (ok true)))
     
;; Add a root-level viewport 
(define-private (wrb-viewport (id uint) (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
   (begin
        (try! (wrb-check-dims start-row start-col num-rows num-cols))
        (asserts! (is-none (map-get? wrb-viewports id)) (err (err-ascii-512 WRB_ERR_EXISTS "viewport already exists")))
        (map-set wrb-viewports
            id
            {
                start-col: start-col,
                start-row: start-row,
                num-cols: num-cols,
                num-rows: num-rows,
                visible: true,
                parent: none,
                last: (var-get wrb-last-viewport-id)
            })
        (var-set wrb-last-viewport-id (some id))
        (ok true)))

;; Add a viewport within an existing viewport
(define-private (wrb-child-viewport (id uint) (parent-id uint) (start-row uint) (start-col uint) (num-rows uint) (num-cols uint))
   (begin
        (try! (wrb-check-dims start-row start-col num-rows num-cols))
        (asserts! (is-none (map-get? wrb-viewports id)) (err (err-ascii-512 WRB_ERR_EXISTS "viewport already exists")))
        (asserts! (is-some (map-get? wrb-viewports parent-id)) (err (err-ascii-512 WRB_ERR_EXISTS "parent viewport does not exist")))
        (map-set wrb-viewports
            id
            {
                start-col: start-col,
                start-row: start-row,
                num-cols: num-cols,
                num-rows: num-rows,
                visible: true,
                parent: (some parent-id),
                last: (var-get wrb-last-viewport-id)
            })
        (var-set wrb-last-viewport-id (some id))
        (ok true)))

;; Fold helper in get-viewports to get the list of viewports
(define-read-only (wrb-get-viewports-iter (ignored bool) (state { cursor: (optional uint), viewports: (list 1024 { id: uint, start-col: uint, start-row: uint, num-cols: uint, num-rows: uint, visible: bool, parent: (optional uint), last: (optional uint) })}))
    (match (get cursor state)
        cursor (let (
            (next-viewport (map-get? wrb-viewports cursor))
            (viewport-list (get viewports state)))
            (match next-viewport
                viewport
                    {
                        cursor: (get last viewport),
                        viewports: (default-to viewport-list (as-max-len? (append viewport-list (merge { id: cursor } viewport)) u1024))
                    }
                state
            ))
        state))

;; Used internally to iterate through viewports 
(define-read-only (wrb-get-viewports (cursor (optional uint)))
    (get viewports (fold wrb-get-viewports-iter (list true true true true true true true true true true true true true true true true true true true true)
        { cursor: (if (is-none cursor) (var-get wrb-last-viewport-id) cursor), viewports: (list ) })))

;; Get a single viewport
(define-read-only (wrb-get-viewport (id uint))
    (match (map-get? wrb-viewports id)
        viewport
            (some (merge viewport { id: id }))
        none))

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb Viewport Settings ;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Viewports changed since last query
(define-data-var viewports-changed (list 1024 uint) (list ))

;; Mark a viewport as updated, so it can be refreshed
(define-private (mark-viewport-updated (id uint))
    (let (
        (update-ids (var-get viewports-changed))
    )
    (var-set viewports-changed (default-to update-ids (as-max-len? (append update-ids id) u1024)))))

;; Set the rows/cols of a viewport
(define-private (wrb-set-viewport-dims (id uint) (rows uint) (cols uint))
    (let (
        (viewport-rec (unwrap! (map-get? wrb-viewports id) (err (err-ascii-512 WRB_ERR_NOT_FOUND "No such viewport"))))
        (new-viewport-rec (merge viewport-rec { num-rows: rows, num-cols: cols })) 
    )
    (try! (wrb-check-dims (get start-row new-viewport-rec) (get start-col new-viewport-rec) (get num-rows new-viewport-rec) (get num-cols new-viewport-rec)))
    (map-set wrb-viewports id (merge viewport-rec { num-rows: rows, num-cols: cols }))
    (mark-viewport-updated id)
    (ok true)))

;; Mark a viewport as visible or invisible.
;; Does not affect child viewports.
;; Returns true if the viewport exists, false if not
(define-private (wrb-viewport-set-visible (id uint) (visible bool))
    (let (
        (updated? (match (map-get? wrb-viewports id)
            viewport (begin
                (map-set wrb-viewports id (merge viewport { visible: visible }))
                true)
            false))
    )
    (if updated?
        (mark-viewport-updated id)
        updated?)))

;;;;;;;;;;;;;;;;;;;;;;;;;; Wrb UI components ;;;;;;;;;;;;;;;;;;;;;;;;;;

(define-map wrb-ui-list
   ;; index
   uint
   ;; element
   {
       viewport: uint,
       type: uint
   })

(define-data-var wrb-ui-list-len uint u0)

(define-map wrb-viewport-text-list
   ;; index
   uint
   ;; payload
   {
       element-id: uint,
       text-handle: uint,
       col: uint,
       row: uint,
       bg-color: uint,
       fg-color: uint
   })

(define-map wrb-viewport-print-list
   ;; index
   uint
   ;; payload
   {
       element-id: uint,
       text-handle: uint,
       cursor: (optional { col: uint, row: uint }),
       bg-color: uint,
       fg-color: uint,
       newline: bool
   })

(define-map wrb-viewport-button-list
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

(define-map wrb-viewport-checkbox-list
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

(define-map wrb-viewport-textline-list
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

(define-map wrb-viewport-textarea-list
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
(define-private (wrb-static-txt-immediate (id uint) (row uint) (col uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; store text
   (wrb-store-large-string-utf8 ui-list-len text)

   ;; add text element
   (map-set wrb-viewport-text-list
       ui-list-len
       { element-id: ui-list-len, row: row, col: col, bg-color: bg-color, fg-color: fg-color, text-handle: ui-list-len })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_TEXT })

   ;; next UI element
   (var-set wrb-ui-list-len (+ u1 ui-list-len))
   true
))

;; Print static text to a viewport, with wordwrap.
(define-private (wrb-inner-static-print-ln-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)) (newline bool))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; store text
   (wrb-store-large-string-utf8 ui-list-len text)

   ;; add text element
   (map-set wrb-viewport-print-list
       ui-list-len
       { element-id: ui-list-len, cursor: cursor, bg-color: bg-color, fg-color: fg-color, text-handle: ui-list-len, newline: newline })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_PRINT })

   ;; next UI element
   (var-set wrb-ui-list-len (+ u1 ui-list-len))
   (if true
       (ok true)
       (err (err-ascii-512 WRB_ERR_INFALLIBLE "infallible")))
))

;; Print static text to a viewport, with wordwrap
(define-private (wrb-static-print-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (wrb-inner-static-print-ln-immediate id cursor bg-color fg-color text false))

;; Print static text to a viewport, with wordwrap and newline
(define-private (wrb-static-println-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (wrb-inner-static-print-ln-immediate id cursor bg-color fg-color text true))

;;;;;;;;;;;;;;;;;;;;;;;; Dynamic UI elements ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

(define-constant MAX_DYNAMIC_UI_ELEMENTS u1024)
(define-data-var wrb-dynamic-element-id uint UPPER_u128)
(define-data-var wrb-dynamic-text (list 1024
    {
        viewport-id: uint,
        element-id: uint,
        row: uint,
        col: uint,
        bg-color: uint,
        fg-color: uint,
        text-handle: uint
    })
    (list ))

(define-data-var wrb-dynamic-prints (list 1024
    {
        viewport-id: uint,
        element-id: uint,
        cursor: (optional { col: uint, row: uint }),
        bg-color: uint,
        fg-color: uint,
        text-handle: uint,
        newline: bool
    })
    (list ))

;; Print dynamic text to a viewport
(define-private (wrb-txt-immediate (id uint) (row uint) (col uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (let (
        (element-id (+ u1 (var-get wrb-dynamic-element-id)))
        (ui-list (var-get wrb-dynamic-text))
        (new-ui-list 
            (default-to ui-list (as-max-len? (append ui-list {
               viewport-id: id,
               element-id: element-id,
               row: row,
               col: col,
               bg-color: bg-color,
               fg-color: fg-color,
               text-handle: element-id
           }) u1024)))
    )
    (if (< (len ui-list) MAX_DYNAMIC_UI_ELEMENTS)
       (begin
           (wrb-store-large-string-utf8 element-id text)
           (var-set wrb-dynamic-element-id element-id)
           (var-set wrb-dynamic-text new-ui-list)
           true)
        false)))

;; Print dynamic text to a viewport, with wordrap and newline
(define-private (wrb-inner-print-ln-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)) (newline bool))
    (let (
        (element-id (+ u1 (var-get wrb-dynamic-element-id)))
        (ui-list (var-get wrb-dynamic-prints))
        (new-ui-list
            (default-to ui-list (as-max-len? (append ui-list {
               viewport-id: id,
               element-id: element-id,
               cursor: cursor,
               bg-color: bg-color,
               fg-color: fg-color,
               text-handle: element-id,
               newline: newline
            }) u1024)))
    )
    (if (< (len ui-list) MAX_DYNAMIC_UI_ELEMENTS)
       (begin
           (wrb-store-large-string-utf8 element-id text)
           (var-set wrb-dynamic-element-id element-id)
           (var-set wrb-dynamic-prints new-ui-list)
           true
       )
       false)))

;; Print dynamic text to a viewport, with wordwrap.
(define-private (wrb-print-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (wrb-inner-print-ln-immediate id cursor bg-color fg-color text false))

;; Print dynamic text to a viewport, with wordwrap and newline.
(define-private (wrb-println-immediate (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (wrb-inner-print-ln-immediate id cursor bg-color fg-color text true))

;;;;;;;;;;;;;;;;;;;;;;;; Viewport Text Elements ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; text colors
(define-map wrb-viewport-static-txt-colors
    uint
    { fg: uint, bg: uint })

(define-map wrb-viewport-txt-colors
    uint
    { fg: uint, bg: uint })

(define-private (wrb-set-static-txt-colors (viewport-id uint) (fg uint) (bg uint))
    (begin
        (map-set wrb-viewport-static-txt-colors viewport-id { fg: fg, bg: bg })
        (ok true)))

(define-private (wrb-set-txt-colors (viewport-id uint) (fg uint) (bg uint))
    (begin
        (map-set wrb-viewport-txt-colors viewport-id { fg: fg, bg: bg })
        (ok true)))

(define-read-only (wrb-get-static-txt-colors (viewport-id uint))
    (default-to { fg: u16777215, bg: u0 }
        (map-get? wrb-viewport-static-txt-colors viewport-id)))

(define-read-only (wrb-get-txt-colors (viewport-id uint))
    (default-to { fg: u16777215, bg: u0 }
        (map-get? wrb-viewport-txt-colors viewport-id)))

(define-private (wrb-static-txt (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (let (
        (colors (wrb-get-static-txt-colors id))
    )
    (wrb-static-txt-immediate id row col (get bg colors) (get fg colors) text)))

(define-private (wrb-static-print (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (let (
        (colors (wrb-get-static-txt-colors id))
    )
    (wrb-static-print-immediate id cursor (get bg colors) (get fg colors) text)))

(define-private (wrb-static-println (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (let (
        (colors (wrb-get-static-txt-colors id))
    )
    (wrb-static-println-immediate id cursor (get bg colors) (get fg colors) text)))

(define-private (wrb-txt (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (let (
        (colors (wrb-get-txt-colors id))
    )
    (wrb-txt-immediate id row col (get bg colors) (get fg colors) text)))

(define-private (wrb-print (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (let (
        (colors (wrb-get-txt-colors id))
    )
    (wrb-print-immediate id cursor (get bg colors) (get fg colors) text)))

(define-private (wrb-println (id uint) (cursor (optional { col: uint, row: uint })) (text (string-utf8 12800)))
    (let (
        (colors (wrb-get-txt-colors id))
    )
    (wrb-println-immediate id cursor (get bg colors) (get fg colors) text)))

(define-data-var wrb-clear-target-viewport-id uint u0)
(define-private (wrb-clear-viewport-dynamic-text
    (text-elem
    {
        viewport-id: uint,
        element-id: uint,
        row: uint,
        col: uint,
        bg-color: uint,
        fg-color: uint,
        text-handle: uint
    }))
    (not (is-eq (get viewport-id text-elem) (var-get wrb-clear-target-viewport-id))))

(define-private (wrb-clear-viewport-dynamic-prints
    (print-elem
    {
        viewport-id: uint,
        element-id: uint,
        cursor: (optional { col: uint, row: uint }),
        bg-color: uint,
        fg-color: uint,
        text-handle: uint,
        newline: bool
    }))
    (not (is-eq (get viewport-id print-elem) (var-get wrb-clear-target-viewport-id))))

;; Clear a viewport of text
;; TODO: set/clear a dirty bit for each element so we don't do it gratuitously
(define-private (wrb-viewport-clear (id uint))
   (begin
      (var-set wrb-clear-target-viewport-id id)
      (let (
          (text-elems (var-get wrb-dynamic-text))
          (print-elems (var-get wrb-dynamic-prints))
          (reduced-text-elems (filter wrb-clear-viewport-dynamic-text text-elems))
          (reduced-print-elems (filter wrb-clear-viewport-dynamic-prints print-elems))
      )
      (var-set wrb-dynamic-text reduced-text-elems)
      (var-set wrb-dynamic-prints reduced-print-elems))
      true))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Viewport UI elements  ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Default button style
(define-data-var wrb-default-button-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-button-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })

;; Add a button to a viewport
;; Returns the button ID
(define-private (wrb-button (id uint) (row uint) (col uint) (text (string-utf8 12800)))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (button-color (var-get wrb-default-button-colors))
        (focused-button-color (var-get wrb-default-focused-button-colors))
    )
    ;; add button element
    (map-set wrb-viewport-button-list
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
    ui-list-len
))

;; Default checkbox style
(define-data-var wrb-default-checkbox-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-checkbox-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })
(define-data-var wrb-default-checkbox-selector-color uint u16711680)

;; Add a checkbox group to a viewport
;; Returns the checkbox ID
(define-private (wrb-checkbox (id uint) (row uint) (col uint) (options (list 256 { text: (string-utf8 200), selected: bool })))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (checkbox-color (var-get wrb-default-checkbox-colors))
        (focused-checkbox-color (var-get wrb-default-focused-checkbox-colors))
        (selector-color (var-get wrb-default-checkbox-selector-color))
    )
    ;; add checkbox element
    (map-set wrb-viewport-checkbox-list
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
    ui-list-len
))

;; Default text line style
(define-data-var wrb-default-textline-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-textline-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })

;; Add a textline to a viewport
;; Returns the textline ID
(define-private (wrb-textline (id uint) (row uint) (col uint) (max-len uint) (text (string-utf8 12800)))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (textline-color (var-get wrb-default-textline-colors))
        (focused-textline-color (var-get wrb-default-focused-textline-colors))
    )
    ;; add textline element
    (map-set wrb-viewport-textline-list
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
    ui-list-len
)) 

;; Default text area style
(define-data-var wrb-default-textarea-colors { fg: uint, bg: uint } { fg: u0, bg: u16776960 })
(define-data-var wrb-default-focused-textarea-colors { fg: uint, bg: uint } { fg: u0, bg: u16711935 })

;; Add a textarea to a viewport
;; Returns the textarea ID
(define-private (wrb-textarea (id uint) (row uint) (col uint) (num-rows uint) (num-cols uint) (max-len uint) (text (string-utf8 12800)))
    (let (
        (ui-list-len (var-get wrb-ui-list-len))
        (textarea-color (var-get wrb-default-textarea-colors))
        (focused-textarea-color (var-get wrb-default-focused-textarea-colors))
    )
    ;; add textarea element
    (map-set wrb-viewport-textarea-list
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
    ui-list-len
)) 

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Viewport UI queries  ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Get the number of UI elements
(define-read-only (wrb-ui-len)
   (var-get wrb-ui-list-len))

;; Get a UI element descriptor
(define-read-only (wrb-ui-element-descriptor (index uint))
   (map-get? wrb-ui-list index))

;; Get a text element
(define-read-only (wrb-ui-get-text-element (index uint))
   (map-get? wrb-viewport-text-list index))

;; Get a print element
(define-read-only (wrb-ui-get-print-element (index uint))
   (map-get? wrb-viewport-print-list index))

;; Get a button element
(define-read-only (wrb-ui-get-button-element (index uint))
   (map-get? wrb-viewport-button-list index))

;; Get a checkbox element
(define-read-only (wrb-ui-get-checkbox-element (index uint))
   (map-get? wrb-viewport-checkbox-list index))

;; Get a textline element
(define-read-only (wrb-ui-get-textline-element (index uint))
   (map-get? wrb-viewport-textline-list index))

;; Get a textarea element
(define-read-only (wrb-ui-get-textarea-element (index uint))
   (map-get? wrb-viewport-textarea-list index))

;; Get all dynamic text statements
(define-read-only (wrb-dynamic-ui-get-text-elements)
    (var-get wrb-dynamic-text))

;; Get all dynamic print statements
(define-read-only (wrb-dynamic-ui-get-print-elements)
    (var-get wrb-dynamic-prints))

;; Get all updated viewports and clear the list
(define-private (wrb-take-viewport-updates)
    (let (
        (update-ids (var-get viewports-changed))
    )
    (var-set viewports-changed (list ))
    update-ids))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Wrbpods  ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Get the address of the user's configured wrbpod
(define-private (wrbpod-default)
    (begin
        (unwrap-panic (contract-call? .wrb-ll wrb-ll-wrbpod-default))
        (unwrap-panic (contract-call? .wrb-ll wrb-ll-get-last-wrbpod-default))))

;; Open a wrbpod. Creates a session for it and returns the session ID (as a uint)
(define-private (wrbpod-open (superblock { contract: principal, slot: uint }))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-open superblock))
        (contract-call? .wrb-ll wrb-ll-get-last-wrbpod-open-result)))

;; How many slots are allocated to this app in the wrbpod?
;; Get the number of slots that the app owns.
;; Returns (response uint { code: uint, message: (string-ascii 512) })
(define-private (wrbpod-get-num-slots (session-id uint) (app-name { name: (buff 48), namespace: (buff 20) }))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-get-num-slots session-id app-name))
        (contract-call? .wrb-ll wrb-ll-get-last-wrbpod-get-num-slots)))

;; Allocate slots in a wrbpod that the user owns
;; Returns (response bool { code: uint, message: (string-ascii 512) }), where
;; (ok true) indicates successful allocation and
;; (ok false) indicates a failure to allocate.
(define-private (wrbpod-alloc-slots (session-id uint) (num-slots uint))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-alloc-slots session-id num-slots))
        (contract-call? .wrb-ll wrb-ll-get-last-wrbpod-alloc-slots-result)))

;; Download a local copy of a wrbpod slot for editing.
;; Slots are 0-indexed from 0 inclusive to the number of slots obtained
;; by (wrbpod-get-num-slots) exclusive.
;; The slot cannot be directly edited; instead, the app uses
;; the (wrbpod-get-slice) and (wrbpod-put-slice) functions to 
;; load and store indexed bytestrings within the slot, respectively.
;; Returns (response { version: uint, signer: (optional principal) } { code: uint, message: (string-ascii 512)})
(define-private (wrbpod-fetch-slot (session-id uint) (slot-id uint))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-fetch-slot session-id slot-id))
        (contract-call? .wrb-ll wrb-ll-get-wrbpod-fetch-slot-result session-id slot-id)))

;; Get a slice of a locally-fetched slot.
(define-private (wrbpod-get-slice (session-id uint) (slot-id uint) (slice-id uint))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-get-slice session-id slot-id slice-id))
        (contract-call? .wrb-ll wrb-ll-get-wrbpod-get-slice-result session-id slot-id slice-id)))

;; Put a slice into a locally-fetched slot, but don't upload it yet
;; The slice won't be persisted until a subsequent call to wrbpod-sync-slot.
(define-private (wrbpod-put-slice (session-id uint) (slot-id uint) (slice-id uint) (data-slice (buff 786000)))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-put-slice session-id slot-id slice-id data-slice))
        (contract-call? .wrb-ll wrb-ll-get-wrbpod-put-slice-result session-id slot-id slice-id)))

;; Synchronize a dirty slot
(define-private (wrbpod-sync-slot (session-id uint) (slot-id uint))
    (begin
        (try! (contract-call? .wrb-ll wrb-ll-wrbpod-sync-slot session-id slot-id))
        (contract-call? .wrb-ll wrb-ll-get-last-wrbpod-sync-slot-result session-id slot-id)))

;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;; Event loop ;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;;

;; Register the main event loop
(define-data-var wrb-event-loop-name (optional (string-ascii 512)) none)
(define-private (wrb-event-loop (function-name (string-ascii 512)))
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
(define-private (wrb-event-subscribe (event-id uint))
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
(define-private (wrb-event-loop-time (delay uint))
    (ok (var-set wrb-event-loop-delay delay)))
(define-read-only (wrb-get-event-loop-time)
    (var-get wrb-event-loop-delay))

(begin
   (print "wrb is not the web"))
