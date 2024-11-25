;; Low-level wrb code.  Do not call directly.

;; The domain this VM instance is for.
;; Called on boot code installation
(define-data-var vm-app-name { name: (buff 48), namespace: (buff 20) } { name: 0x, namespace: 0x })
(define-private (set-app-name (app-name { name: (buff 48), namespace: (buff 20) }))
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

;; Code that the wrb special case handler uses to load and store a call-readonly result
;; into the boot code, for consumption via the public API.  This function is intercepted.
(define-data-var last-call-readonly (response (buff 102400) (string-ascii 512)) (ok 0x))
(define-public (call-readonly (contract principal) (function-name (string-ascii 128)) (function-args-list (buff 102400)))
   (ok 0x))
(define-private (set-last-call-readonly (result (response (buff 102400) (string-ascii 512))))
   (ok (var-set last-call-readonly result)))
(define-public (get-last-call-readonly)
   (var-get last-call-readonly))

;; Code that the wrb special case handler uses to get an attachment from the node.
;; This function is intercepted.
(define-data-var last-attachment (response (buff 102400) (string-ascii 512)) (ok 0x))
(define-public (get-attachment (attachment-hash (buff 20)))
   (ok 0x))
(define-private (set-last-attachment (result (response (buff 102400) (string-ascii 512))))
   (ok (var-set last-attachment result)))
(define-public (get-last-attachment)
   (var-get last-attachment))

;; Code that the wrb special case handler uses to load and store a buff-to-string-utf8 value
;; into the boot code, for consumption by the public API.  This function is intercepted
(define-public (buff-to-string-utf8 (arg (buff 102400)))
   (ok true))

(define-data-var last-wrb-buff-to-string-utf8 (response (string-utf8 25600) (string-ascii 512)) (ok u""))
(define-private (set-last-wrb-buff-to-string-utf8 (conv-res (response (string-utf8 25600) (string-ascii 512))))
   (ok (var-set last-wrb-buff-to-string-utf8 conv-res)))
(define-public (get-last-wrb-buff-to-string-utf8)
   (var-get last-wrb-buff-to-string-utf8))

(define-private (ascii-512 (str (string-ascii 512)))
    (unwrap-panic (as-max-len? str u512)))

;; Code that the wrb special case handler uses to open a stackerdb session
(define-map wrbpod-sessions
    ;; session ID
    uint
    ;; session data
    {
        contract-id: principal,
        owned: bool
    })

(define-map wrbpod-session-contracts
    ;; contract opened
    principal
    ;; session ID
    uint
)

(define-data-var last-wrbpod-session-result (response (optional uint) (string-ascii 512)) (ok none))
(define-data-var last-wrbpod-session uint u0)

(define-read-only (get-last-wrbpod-open-result)
    (match (var-get last-wrbpod-session-result)
        ok-opt (ok (unwrap-panic ok-opt))
        err-str (err err-str)))

;; called internally
(define-private (finish-wrbpod-open (stackerdb-contract principal) (wrbpod-open-res (response bool (string-ascii 512))))
    (let (
        (wrbpod-session (+ u1 (var-get last-wrbpod-session)))
    )
        (match wrbpod-open-res
            is-owned
               (begin
                   (var-set last-wrbpod-session wrbpod-session)
                   (var-set last-wrbpod-session-result (ok (some wrbpod-session)))
                   (map-set wrbpod-sessions wrbpod-session { owned: is-owned, contract-id: stackerdb-contract })
                   (map-set wrbpod-session-contracts stackerdb-contract wrbpod-session)
                   (ok wrbpod-session))
            err-res
                (begin
                   (var-set last-wrbpod-session-result (err err-res))
                   (err err-res)))
    ))

;; This is intercepted
(define-public (wrbpod-open (stackerdb-contract principal))
    (let (
        (is-contract (is-some
            (match (principal-destruct? stackerdb-contract)
                ok-contract-parts (get name ok-contract-parts)
                err-contract-parts (get name err-contract-parts))))
    )
        (asserts! is-contract
            (err (ascii-512 "principal is not a contract")))

        (ok (map-get? wrbpod-session-contracts stackerdb-contract))
    ))

;; Code that the wrb special case handler uses to allocate slots in the user's wrbpod
(define-data-var last-wrbpod-alloc-slots-result (response bool (string-ascii 512)) (ok false))
(define-private (set-last-wrbpod-alloc-slots-result (res (response bool (string-ascii 512))))
    (ok (var-set last-wrbpod-alloc-slots-result res)))
(define-read-only (get-last-wrbpod-alloc-slots-result)
    (var-get last-wrbpod-alloc-slots-result))

;; this is intercepted
(define-public (wrbpod-alloc-slots (session-id uint) (num-slots uint))   
    (begin
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (ascii-512 "no such session")))

        (ok true)))

(define-data-var last-wrbpod-get-num-slots-result (response uint (string-ascii 512)) (ok u0))
(define-private (set-last-wrbpod-get-num-slots (num-slots-res (response uint (string-ascii 512))))
    (ok (var-set last-wrbpod-get-num-slots-result num-slots-res)))
(define-public (get-last-wrbpod-get-num-slots)
    (var-get last-wrbpod-get-num-slots-result))

;; this is intercepted
(define-public (wrbpod-get-num-slots (session-id uint) (app-name { name: (buff 48), namespace: (buff 20) }))
    (begin
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (ascii-512 "no such session")))

        (ok u0)))

;; Fetched slots. The data is stored internally.
(define-map last-wrbpod-fetch-slot-results
    { session-id: uint, slot-id: uint }
    (response { version: uint, signer: (optional (buff 33)) } (string-ascii 512)))

(define-private (set-last-wrbpod-fetch-slot-result (session-id uint) (slot-id uint) (result (response { version: uint, signer: (optional (buff 33)) } (string-ascii 512))))
    (ok (map-set last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id } result)))

(define-read-only (get-wrbpod-fetch-slot-result (session-id uint) (slot-id uint))
    (default-to (err (ascii-512 "no such slot in session")) (map-get? last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id })))

;; Code that the wrb special case handler uses to fetch a wrbpod slot.
;; This is intercepted
(define-public (wrbpod-fetch-slot (session-id uint) (slot-id uint))
    (begin
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (ascii-512 "no such session")))

        (ok { version: u0, signer: none })))

;; Code that the wrb special case handler uses to fetch a slice
(define-map last-wrbpod-get-slice-results
    { session-id: uint, slot-id: uint, slice-id: uint }
    ;; the response
    (response (buff 786000) (string-ascii 512)))

;; called internally to store a loaded slice
(define-private (set-last-wrbpod-get-slice-result (session-id uint) (slot-id uint) (slice-id uint) (res (response (buff 786000) (string-ascii 512))))
    (ok (map-set last-wrbpod-get-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id } res)))

(define-public (get-wrbpod-get-slice-result (session-id uint) (slot-id uint) (slice-id uint))
    (default-to
        (err (ascii-512 "no such slice loaded in given slot and session"))
        (map-get? last-wrbpod-get-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id })))

;; this is intercepted
(define-public (wrbpod-get-slice (session-id uint) (slot-id uint) (slice-id uint))
    (begin
        ;; we must already have a session to this wrbpod
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (ascii-512 "no such session")))

        ;; we must already have this slot
        (try! (match (map-get? last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id })
            slot-result
                (ok
                    (asserts! (is-ok slot-result)
                        (err (ascii-512 "cannot get slice from failed slot"))))
            (err (ascii-512 "no such opened slot"))))
       
        (ok none))) 

(define-map last-wrbpod-put-slice-results
    { session-id: uint, slot-id: uint, slice-id: uint }
    (response bool (string-ascii 512)))

(define-private (set-last-wrbpod-put-slice-result (session-id uint) (slot-id uint) (slice-id uint) (res (response bool (string-ascii 512))))
    (ok (map-set last-wrbpod-put-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id } res)))

(define-public (get-wrbpod-put-slice-result (session-id uint) (slot-id uint) (slice-id uint))
    (default-to
        (err (ascii-512 "no such slice stored in given slot and session"))
        (map-get? last-wrbpod-put-slice-results { session-id: session-id, slot-id: slot-id, slice-id: slice-id })))

;; this is intercepted
(define-public (wrbpod-put-slice (session-id uint) (slot-id uint) (slice-id uint) (slice-data (buff 786000)))
    ;; make it so this type is fully known
    (if true
        (ok true)
        (err (ascii-512 "unreachable"))))

(define-map last-wrbpod-sync-slot-results
    { session-id: uint, slot-id: uint }
    (response bool (string-ascii 512)))

(define-private (set-last-wrbpod-sync-slot-result (session-id uint) (slot-id uint) (res (response bool (string-ascii 512))))
    (ok (map-set last-wrbpod-sync-slot-results { session-id: session-id, slot-id: slot-id } res)))

(define-public (get-last-wrbpod-sync-slot-result (session-id uint) (slot-id uint))
    (default-to (ok true) (map-get? last-wrbpod-sync-slot-results { session-id: session-id, slot-id: slot-id })))
 
;; This is intercepted.
(define-public (wrbpod-sync-slot (session-id uint) (slot-id uint))
    (begin
        ;; we must already have a session to this wrbpod
        (asserts! (is-some (map-get? wrbpod-sessions session-id))
            (err (ascii-512 "No such session")))
        
        ;; we must already have this slot
        (try! (match (map-get? last-wrbpod-fetch-slot-results { session-id: session-id, slot-id: slot-id })
            slot-result
                (ok
                    (asserts! (is-ok slot-result)
                        (err (ascii-512 "cannot get slice from failed slot"))))
            (err (ascii-512 "no such opened slot"))))
       
        (ok true)))
