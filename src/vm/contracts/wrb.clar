;; Returns a BIP39 seed phrase, or an error message.
;; Up to 144 characters long.
(define-public (seed-phrase)
   (begin
       (unwrap-panic (contract-call? .wrb-ll generate-wrb-seed-phrase))
       (contract-call? .wrb-ll get-last-wrb-seed-phrase)))

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
(define-public (wrb-root (cols uint) (rows uint))
   (ok (var-set wrb-root-size { cols: cols, rows: rows })))

;; Get the root pane's dimensions
(define-read-only (get-wrb-root)
   (var-get wrb-root-size))

(define-data-var viewports (list 256 {
    id: uint,
    start-col: uint,
    start-row: uint,
    num-cols: uint,
    num-rows: uint,
    visible: bool
}) (list ))

(define-private (ascii-512 (str (string-ascii 512)))
    (unwrap-panic (as-max-len? str u512)))

;; Add a viewport 
(define-public (wrb-viewport (id uint) (start-col uint) (start-row uint) (num-cols uint) (num-rows uint))
   (begin
        (asserts! (< start-col u65536) (err (ascii-512 "start-col too big")))
        (asserts! (< start-row u65536) (err (ascii-512 "start-row too big")))
        (asserts! (< (+ start-col num-cols) u65536) (err (ascii-512 "num-cols too big")))
        (asserts! (< (+ start-row num-rows) u65536) (err (ascii-512 "num-rows too big")))
        (let (
            (cur-viewports (var-get viewports))
            (new-viewports
                (unwrap!
                    (as-max-len?
                       (append cur-viewports {
                           id: id,
                           start-col: start-col,
                           start-row: start-row,
                           num-cols: num-cols,
                           num-rows: num-rows,
                           visible: true
                       })
                    u256)
                (err (ascii-512 "FATAL: too many viewports"))))
       )
           (var-set viewports new-viewports)
           (ok true)
       )))

(define-read-only (get-viewports)
   (var-get viewports))

;; UI element type IDs
(define-constant WRB_UI_TYPE_TEXT u0)
(define-constant WRB_UI_TYPE_PRINT u1)

(define-map wrb-ui-list
   ;; index
   uint
   ;; element
   {
       viewport: uint,
       type: uint,
   })

(define-data-var wrb-ui-list-len uint u0)

(define-map viewport-text-list
   ;; index
   uint
   ;; payload
   {
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
       text: (string-utf8 12800),
       cursor: (optional { col: uint, row: uint }),
       bg-color: uint,
       fg-color: uint,
       newline: bool
   })

;; Add raw text to a viewport
(define-public (wrb-viewport-add-text (id uint) (col uint) (row uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; add text element
   (map-set viewport-text-list
       ui-list-len
       { row: row, col: col, bg-color: bg-color, fg-color: fg-color, text: text })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_TEXT })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok true)
))

;; Print text to a viewport, with wordwrap.
(define-public (wrb-viewport-print (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; add text element
   (map-set viewport-print-list
       ui-list-len
       { cursor: cursor, bg-color: bg-color, fg-color: fg-color, text: text, newline: false })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_PRINT })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok true)
))

;; Print text to a viewport, with wordwrap and newline
(define-public (wrb-viewport-println (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
   (let (
       (ui-list-len (var-get wrb-ui-list-len))
   )
   ;; add text element
   (map-set viewport-print-list
       ui-list-len
       { cursor: cursor, bg-color: bg-color, fg-color: fg-color, text: text, newline: true })

   ;; register UI element
   (map-set wrb-ui-list
       ui-list-len
       { viewport: id, type: WRB_UI_TYPE_PRINT })

    ;; next UI element
    (var-set wrb-ui-list-len (+ u1 ui-list-len))
    (ok true)
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

(begin
   (print "wrb is not the web"))
