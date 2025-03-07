;; Low-level wrb code.  Do not call directly.

(define-constant WRB_ERR_INFALLIBLE u0)
(define-constant WRB_ERR_INVALID u1)

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

;; The domain this VM instance is for.
;; Called on boot code installation
(define-data-var vm-app-name { name: (buff 48), namespace: (buff 20), version: uint } { name: 0x, namespace: 0x, version: u0 })
(define-private (set-app-name (app-name { name: (buff 48), namespace: (buff 20), version: uint }))
    (var-set vm-app-name app-name))
(define-read-only (get-app-name)
    (var-get vm-app-name))

;; The version of the code running.
;; Called on page load
(define-data-var vm-app-code-hash (buff 20) 0x)
(define-private (set-app-code-hash (hash (buff 20)))
    (var-set vm-app-code-hash hash))
(define-read-only (get-app-code-hash)
    (var-get vm-app-code-hash))

;; Error constructor
(define-private (err-ascii-512 (code uint) (str (string-ascii 512)))
    { code: code, message: (unwrap-panic (as-max-len? str u512)) })

;; Code that the wrb special case handler uses to load and store a call-readonly result
;; into the boot code, for consumption via the public API.  This function is intercepted.
(define-data-var last-call-readonly (response (buff 102400) { code: uint, message: (string-ascii 512) }) (ok 0x))
(define-public (call-readonly (contract principal) (function-name (string-ascii 128)) (function-args-list (buff 102400)))
   (ok 0x))
(define-private (set-last-call-readonly (result (response (buff 102400) { code: uint, message: (string-ascii 512) })))
   (ok (var-set last-call-readonly result)))
(define-public (get-last-call-readonly)
   (var-get last-call-readonly))

;; Code that the wrb special case handler uses to load and store a buff-to-string-utf8 value
;; into the boot code, for consumption by the public API.  This function is intercepted
(define-public (buff-to-string-utf8 (arg (buff 102400)))
   (ok true))

(define-data-var last-wrb-buff-to-string-utf8 (response (string-utf8 25600) { code: uint, message: (string-ascii 512) }) (ok u""))
(define-private (set-last-wrb-buff-to-string-utf8 (conv-res (response (string-utf8 25600) { code: uint, message: (string-ascii 512) })))
   (ok (var-set last-wrb-buff-to-string-utf8 conv-res)))
(define-public (get-last-wrb-buff-to-string-utf8)
   (var-get last-wrb-buff-to-string-utf8))

;; Code that the wrb special case handler uses to load and store a string-ascii-to-string-utf8 value
;; into the boot code, for consumption by the public API.  This function is intercepted
(define-public (string-ascii-to-string-utf8 (arg (string-ascii 25600)))
   (ok true))

(define-data-var last-wrb-string-ascii-to-string-utf8 (response (string-utf8 25600) { code: uint, message: (string-ascii 512) }) (ok u""))
(define-private (set-last-wrb-string-ascii-to-string-utf8 (conv-res (response (string-utf8 25600) { code: uint, message: (string-ascii 512) })))
   (ok (var-set last-wrb-string-ascii-to-string-utf8 conv-res)))
(define-public (get-last-wrb-string-ascii-to-string-utf8)
   (var-get last-wrb-string-ascii-to-string-utf8))


;; Code that the wrb special case handler uses to open a stackerdb session
(define-map wrbpod-sessions
    ;; session ID
    uint
    ;; session data
    {
        superblock: { contract: principal, slot: uint },
        owned: bool
    })

(define-data-var last-wrbpod-default {contract: principal, slot: uint} {contract: 'SP000000000000000000002Q6VF78.wrb, slot: u0})

;; This is intercepted
(define-public (wrbpod-default)
    (if true
        (ok { contract: 'SP000000000000000000002Q6VF78.wrb, slot: u0 })
        (err (err-ascii-512 WRB_ERR_INFALLIBLE "Infallible"))))

(define-private (finish-wrbpod-default (cfg-wrbpod-default { contract: principal, slot: uint }))
    (begin
        (var-set last-wrbpod-default cfg-wrbpod-default)
        (ok true)))

(define-read-only (get-last-wrbpod-default)
    (if true
        (ok (var-get last-wrbpod-default))
        (err (err-ascii-512 WRB_ERR_INFALLIBLE "Infallible"))))

(define-data-var last-wrbpod-session-result (response uint { code: uint, message: (string-ascii 512) }) (err (err-ascii-512 WRB_ERR_WRBPOD_NOT_OPEN "No successful open yet")))

(define-read-only (get-last-wrbpod-open-result)
    (var-get last-wrbpod-session-result))

;; called internally
(define-private (finish-wrbpod-open (superblock { contract: principal, slot: uint }) (wrbpod-session uint) (wrbpod-open-res (response bool { code: uint, message: (string-ascii 512) })))
     (match wrbpod-open-res
         is-owned
            (begin
                (var-set last-wrbpod-session-result (ok wrbpod-session))
                (map-set wrbpod-sessions wrbpod-session { owned: is-owned, superblock: superblock })
                (ok wrbpod-session))
         err-res
             (begin
                (var-set last-wrbpod-session-result (err err-res))
                (err err-res))))

;; This is intercepted
(define-public (wrbpod-open (superblock { contract: principal, slot: uint }))
    (let (
        (is-contract (is-some
            (match (principal-destruct? (get contract superblock))
                ok-contract-parts (get name ok-contract-parts)
                err-contract-parts (get name err-contract-parts))))
    )
        (asserts! is-contract
            (err (err-ascii-512 WRB_ERR_INVALID "principal is not a contract")))

        (ok u0)
    ))

;; Code that the wrb special case handler uses to allocate slots in the user's wrbpod
(define-data-var last-wrbpod-alloc-slots-result (response bool { code: uint, message: (string-ascii 512) }) (ok false))
(define-private (set-last-wrbpod-alloc-slots-result (res (response bool { code: uint, message: (string-ascii 512) })))
    (ok (var-set last-wrbpod-alloc-slots-result res)))
(define-read-only (get-last-wrbpod-alloc-slots-result)
    (var-get last-wrbpod-alloc-slots-result))

;; this is intercepted
(define-public (wrbpod-alloc-slots (session-id uint) (num-slots uint))   
    (begin
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (err-ascii-512 WRB_ERR_WRBPOD_NOT_OPEN "no such session")))

        (ok true)))

(define-data-var last-wrbpod-get-num-slots-result (response uint { code: uint, message: (string-ascii 512) }) (ok u0))
(define-private (set-last-wrbpod-get-num-slots (num-slots-res (response uint { code: uint, message: (string-ascii 512) })))
    (ok (var-set last-wrbpod-get-num-slots-result num-slots-res)))
(define-public (get-last-wrbpod-get-num-slots)
    (var-get last-wrbpod-get-num-slots-result))

;; this is intercepted
(define-public (wrbpod-get-num-slots (session-id uint) (app-name { name: (buff 48), namespace: (buff 20) }))
    (begin
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (err-ascii-512 WRB_ERR_WRBPOD_NOT_OPEN "no such session")))

        (ok u0)))

;; Fetched slots. The data is stored internally.
(define-map last-wrbpod-fetch-slot-results
    { session-id: uint, slot-id: uint }
    (response { version: uint, signer: (optional (buff 33)) } { code: uint, message: (string-ascii 512) }))

(define-private (set-last-wrbpod-fetch-slot-result (session-id uint) (slot-id uint) (result (response { version: uint, signer: (optional (buff 33)) } { code: uint, message: (string-ascii 512) })))
    (ok (map-set last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id } result)))

(define-read-only (get-wrbpod-fetch-slot-result (session-id uint) (slot-id uint))
    (default-to (err (err-ascii-512 WRB_ERR_WRBPOD_NO_SLOT "no such slot in session")) (map-get? last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id })))

;; Code that the wrb special case handler uses to fetch a wrbpod slot.
;; This is intercepted
(define-public (wrbpod-fetch-slot (session-id uint) (slot-id uint))
    (begin
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (err-ascii-512 WRB_ERR_WRBPOD_NOT_OPEN "no such session")))

        (ok { version: u0, signer: none })))

;; Code that the wrb special case handler uses to fetch a slice
(define-map last-wrbpod-get-slice-results
    { session-id: uint, slot-id: uint, slice-id: uint }
    ;; the response
    (response (buff 786000) { code: uint, message: (string-ascii 512) }))

;; called internally to store a loaded slice
(define-private (set-last-wrbpod-get-slice-result (session-id uint) (slot-id uint) (slice-id uint) (res (response (buff 786000) { code: uint, message: (string-ascii 512) })))
    (ok (map-set last-wrbpod-get-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id } res)))

(define-public (get-wrbpod-get-slice-result (session-id uint) (slot-id uint) (slice-id uint))
    (default-to
        (err (err-ascii-512 WRB_ERR_WRBPOD_NO_SLICE "no such slice loaded in given slot and session"))
        (map-get? last-wrbpod-get-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id })))

;; this is intercepted
(define-public (wrbpod-get-slice (session-id uint) (slot-id uint) (slice-id uint))
    (begin
        ;; we must already have a session to this wrbpod
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (err-ascii-512 WRB_ERR_WRBPOD_NOT_OPEN "no such session")))

        ;; we must already have this slot
        (try! (match (map-get? last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id })
            slot-result slot-result
            (err (err-ascii-512 WRB_ERR_WRBPOD_NO_SLOT "no such opened slot"))))
       
        (ok none))) 

(define-map last-wrbpod-put-slice-results
    { session-id: uint, slot-id: uint, slice-id: uint }
    (response bool { code: uint, message: (string-ascii 512) }))

(define-private (set-last-wrbpod-put-slice-result (session-id uint) (slot-id uint) (slice-id uint) (res (response bool { code: uint, message: (string-ascii 512) })))
    (ok (map-set last-wrbpod-put-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id } res)))

(define-public (get-wrbpod-put-slice-result (session-id uint) (slot-id uint) (slice-id uint))
    (default-to
        (err (err-ascii-512 WRB_ERR_WRBPOD_NO_SLICE "no such slice stored in given slot and session"))
        (map-get? last-wrbpod-put-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id })))

;; this is intercepted
(define-public (wrbpod-put-slice (session-id uint) (slot-id uint) (slice-id uint) (slice-data (buff 786000)))
    ;; make it so this type is fully known
    (if true
        (ok true)
        (err (err-ascii-512 WRB_ERR_INFALLIBLE "unreachable"))))

(define-map last-wrbpod-sync-slot-results
    { session-id: uint, slot-id: uint }
    (response bool { code: uint, message: (string-ascii 512) }))

(define-private (set-last-wrbpod-sync-slot-result (session-id uint) (slot-id uint) (res (response bool { code: uint, message: (string-ascii 512) })))
    (ok (map-set last-wrbpod-sync-slot-results { session-id: session-id, slot-id: slot-id } res)))

(define-public (get-last-wrbpod-sync-slot-result (session-id uint) (slot-id uint))
    (default-to (ok true) (map-get? last-wrbpod-sync-slot-results { session-id: session-id, slot-id: slot-id })))
 
;; This is intercepted.
(define-public (wrbpod-sync-slot (session-id uint) (slot-id uint))
    (begin
        ;; we must already have a session to this wrbpod
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (err-ascii-512 WRB_ERR_WRBPOD_NOT_OPEN "No such session")))
        
        ;; we must already have this slot
        (try! (match (map-get? last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id })
            slot-result slot-result
            (err (err-ascii-512 WRB_ERR_WRBPOD_NO_SLOT "no such opened slot"))))
       
        (ok true)))
