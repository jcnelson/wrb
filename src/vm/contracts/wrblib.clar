;; Wrapper around .wrb functions.
;; This gets linked directly into the wrb application.

(define-private (wrb-seed-phrase)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb seed-phrase))

(define-private (wrb-call-readonly? (contract principal) (function-name (string-ascii 128)) (function-args-list (buff 102400)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb call-readonly contract function-name function-args-list))

(define-private (wrb-get-attachment? (attachment-hash (buff 20)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb get-attachment attachment-hash))

(define-private (wrb-buff-to-string-utf8? (arg (buff 102400)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb buff-to-string-utf8 arg))

;; Define the size of the root viewport in rows and columns
(define-private (wrb-root (cols uint) (rows uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-root cols rows))

;; Get the root description
(define-read-only (wrb-get-root)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb get-wrb-root))

;; Declare a viewport
(define-private (wrb-viewport (id uint) (start-col uint) (start-row uint) (num-cols uint) (num-rows uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport id start-col start-row num-cols num-rows))

;; Get the viewports
(define-read-only (wrb-get-viewports)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb get-viewports))

;; Add text to a viewport
(define-private (wrb-raw-txt (id uint) (col uint) (row uint) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-add-text id col row bg-color fg-color text))

;; Print text to a viewport, no newline, with given foreground/background colors and a specific cursor
(define-private (wrb-raw-print (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-print id cursor bg-color fg-color text))

;; Print text to a viewport with newline, with given foreground/background colors and a specific cursor
(define-private (wrb-raw-println (id uint) (cursor (optional { col: uint, row: uint })) (bg-color uint) (fg-color uint) (text (string-utf8 12800)))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-viewport-println id cursor bg-color fg-color text))

;; Get the number of UI elements
(define-read-only (wrb-ui-len)
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-len))

;; Get a UI element descriptor at a particular index
(define-read-only (wrb-ui-element-descriptor (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-element-descriptor index))

;; Get a text UI element at a particular index
(define-read-only (wrb-ui-get-text-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-text-element index))

;; Get a print UI element at a particular index
(define-read-only (wrb-ui-get-print-element (index uint))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrb-ui-get-print-element index))

;; Open a connection to a wrbpod
(define-private (wrbpod-open (stackerdb-contract principal))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-open stackerdb-contract))

;; Get the number of slots that the app owns.
;; Returns (response uint (string-ascii 512))
(define-public (wrbpod-get-num-slots (session-id uint) (app-name { name: (buff 48), namespace: (buff 20) }))
    (contract-call? 'SP000000000000000000002Q6VF78.wrb wrbpod-get-num-slots session-id app-name))

;; Allocate slots in a wrbpod that the user owns
;; Returns (response bool (string-ascii 512)), where
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
;; Returns (response { version: uint, signer: principal } (string-ascii 512))
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

